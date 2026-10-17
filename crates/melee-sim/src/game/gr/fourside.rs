//! Fourside, `melee/gr/grfourside.c`: the city the fighters stand on (map 6), the crane and its
//! girder (map 1), the helicopter (map 3), the UFO (map 5), the background buildings (map 4)
//! and maps 0 and 2.
//!
//! The crane swings its girder over the stage and raises and lowers it now and then. The UFO
//! waits; the longer no one appears for it (and sooner when a fighter stands on the
//! skyscraper's roof) it comes down over one of three buildings, to be stood on, and flies off
//! again. The helicopter crosses behind when the UFO isn't out.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::camera;
use super::super::handles::{CodeRef, GroundId};
use super::super::state::{Game, Running};
use super::data::{FOURSIDE_CALLBACKS, StageCallbacks};
use super::ground::{
    ground_of, jobj_of, map_gobj, nth_joint, set_guide_points, update_coll_joint, update_map_coll,
};
use super::{anime, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

/// `fn_801F3F74`, the city's render callback.
pub const RENDER_CITY: u32 = 0x801F_3F74;

/// `grFourside_801F30A0`, the roof's collision joint's callback: someone stands on it.
pub const ROOF_TOUCHED: u32 = 0x801F_30A0;

/// The UFO's map id and its collision joint.
const MAP_UFO: i32 = 5;
const UFO_JOINT: i32 = 4;

/// The fields of `grFourside_YakumonoParam`, by offset.
mod p {
    pub const HELI_WAIT: u32 = 0;
    pub const HELI_WAIT_ADD: u32 = 4;
    pub const CRANE_WAIT: u32 = 0xC;
    pub const CRANE_WAIT_ADD: u32 = 0x10;
    pub const CRANE_IRON_WAIT: u32 = 0x14;
    pub const CRANE_IRON_WAIT_ADD: u32 = 0x18;
    pub const CRANE_IRON_UP_MIN: u32 = 0x1C;
    pub const CRANE_IRON_UP_MAX: u32 = 0x20;
    pub const CRANE_IRON_DOWN_MIN: u32 = 0x24;
    pub const CRANE_IRON_DOWN_MAX: u32 = 0x28;
    pub const CRANE_IRON_SPD: u32 = 0x2C;
    pub const CRANE_IRON_STOP_ACL: u32 = 0x30;
    pub const UFO_WAIT: u32 = 0x34;
    pub const UFO_CS_OFFS: u32 = 0x38;
    pub const UFO_STAY_TIME: u32 = 0x3C;
    pub const UFO_STAY_TIME_ADD: u32 = 0x40;
    pub const UFO_CHALLENGE: u32 = 0x44;
    pub const X46: u32 = 0x46;
    pub const X48: u32 = 0x48;
}

#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn i(self, r: &Running, off: u32) -> i32 {
        r.data.u32(self.0.at(off)) as i32
    }
    fn h(self, r: &Running, off: u32) -> i16 {
        r.data.u16(self.0.at(off)) as i16
    }
    fn f(self, r: &Running, off: u32) -> f32 {
        r.data.f32(self.0.at(off))
    }
    /// `base + (add != 0 ? HSD_Randi(add) : 0)`.
    fn wait(self, r: &mut Running, base: u32, add: u32) -> i32 {
        let a = self.i(r, add);
        let extra = if a != 0 { r.game.objs.rng.below(a) } else { 0 };
        self.i(r, base).wrapping_add(extra)
    }
}

fn param(game: &Game) -> Param {
    Param(
        game.gr
            .fourside_yakumono
            .expect("grfourside.c: yakumono_param"),
    )
}

/// A stage object's process or callback, by address: Fourside's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x801F_2EF0 | 0x801F_2EF4 | 0x801F_2F2C | 0x801F_2F30 | 0x801F_309C | 0x801F_314C
        | 0x801F_3150 | 0x801F_37F8 | 0x801F_3B6C | 0x801F_3F0C => empty(r, gobj),
        0x801F_2EBC | 0x801F_2EF8 => init_anim(r, gobj),
        0x801F_2F34 => init_city(r, gobj),
        0x801F_3078 => proc_city(r, gobj),
        0x801F_30F0 => init_buildings(r, gobj),
        0x801F_3154 => init_crane(r, gobj),
        0x801F_3274 => proc_crane(r, gobj),
        0x801F_37FC => init_ufo(r, gobj),
        0x801F_3894 => proc_ufo(r, gobj),
        0x801F_3C40 => init_heli(r, gobj),
        0x801F_3CC8 => proc_heli(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x801F_2D10 => on_init(r),
        0x801F_2DA0 => on_load(r),
        0x801F_2DA4 => on_start(r),
        _ => return false,
    }
    true
}

/// The stage's render callbacks, by address.
pub fn run_render(r: &mut Running, gobj: GObjId, cb: u32, pass: i32) -> bool {
    match cb {
        RENDER_CITY => render_city(r, gobj, pass),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grFourside_801F2EF0")]
#[doc(alias = "grFourside_801F2EF4")]
#[doc(alias = "grFourside_801F2F2C")]
#[doc(alias = "grFourside_801F2F30")]
#[doc(alias = "grFourside_801F309C")]
#[doc(alias = "grFourside_801F314C")]
#[doc(alias = "grFourside_801F3150")]
#[doc(alias = "grFourside_801F37F8")]
#[doc(alias = "grFourside_801F3B6C")]
#[doc(alias = "grFourside_801F3F0C")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `grFourside_801F2DD0`.
#[doc(alias = "grFourside_801F2DD0")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grFs_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &FOURSIDE_CALLBACKS[map_id as usize]
}

/// Fourside's setup, `grFourside_801F2D10`: its parameters, maps 0, 4, 6, 2, 1, 3 and 5,
/// camera limits and blast zones.
#[doc(alias = "grFourside_801F2D10")]
pub fn on_init(r: &mut Running) {
    r.game.gr.fourside_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [0, 4, 6, 2, 1, 3, 5] {
        create(r, map_id, setup::SETUP_FRAME);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
}

/// `grFourside_801F2D0C` does nothing (only a demo runs it).
#[doc(alias = "grFourside_801F2D0C")]
pub fn on_demo_init(_r: &mut Running) {}

/// `grFourside_801F2DA0` does nothing.
#[doc(alias = "grFourside_801F2DA0")]
pub fn on_load(_r: &mut Running) {}

/// Fourside's start, `grFourside_801F2DA4`: the enemy spawner, without enemies.
#[doc(alias = "grFourside_801F2DA4")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// Maps 0's and 2's setups, `stageGObj0_OnInit` and `stageGObj2_OnInit`: their animations.
#[doc(alias = "stageGObj0_OnInit@801F2EBC")]
#[doc(alias = "stageGObj2_OnInit@801F2EF8")]
pub fn init_anim(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The city's setup (map 6), `grFourside_801F2F34`: its collision and animation, the guide
/// points, its own render callback turning three layers of the skyline with the camera (two of
/// its joints and the background buildings' model), the roof's callback, and it receives
/// shadows.
#[doc(alias = "grFourside_801F2F34")]
pub fn init_city(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let n = |r: &Running, k: i32| nth_joint(r.game, gobj, k);
    let points = [n(r, 7), n(r, 4), n(r, 6), n(r, 5), n(r, 9), n(r, 8)];
    set_guide_points(r.game, r.data, points);
    r.game.world.gobj_mut(gobj).render = Some(CodeRef(RENDER_CITY));
    let (a, b) = (n(r, 2), n(r, 3));
    let back = map_gobj(r.game, 4).and_then(|b| jobj_of(r.game, b));
    let v = r.game.grounds[g.0].u.fourside();
    v.x0 = a;
    v.x4 = b;
    v.x8 = back;
    r.game.geometry_mut().set_ledge_callback(
        0,
        Some(super::CollUser::GObj(gobj)),
        Some(CodeRef(ROOF_TOUCHED)),
    );
    let gp = &mut r.game.grounds[g.0];
    gp.x10_flags.b5 = true;
    gp.x11_flags.b012 = 1;
}

/// The city's process, `grFourside_801F3078`: the collision follows the model, and the wind
/// sources.
#[doc(alias = "grFourside_801F3078")]
pub fn proc_city(r: &mut Running, gobj: GObjId) {
    update_map_coll(r.game, r.data, gobj);
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
}

/// The roof's collision joint's callback, `grFourside_801F30A0`: a fighter (collision kind 1)
/// on it counts toward the UFO's coming (the UFO's `x8`, which its process clears each tick).
#[doc(alias = "grFourside_801F30A0")]
pub fn roof_touched(gp_ufo: &mut super::super::structs::Ground, kind: u32) {
    if kind == 1 {
        let v = gp_ufo.u.fourside_ufo();
        v.x8 = v.x8.wrapping_add(1);
    }
}

/// [`roof_touched`] by the UFO's object, for callers that hold the stage objects alone.
pub fn touch(
    grounds: &mut crate::hsd::pool::Pool<super::super::structs::Ground>,
    ufo: Option<GroundId>,
    kind: u32,
) {
    let g = ufo.expect("Ground_GetMapGObj(5)");
    roof_touched(&mut grounds[g.0], kind);
}

/// The background buildings' setup (map 4), `grFourside_801F30F0`: no `x10` bit 2, its
/// animation, drawn in the background layer.
#[doc(alias = "grFourside_801F30F0")]
pub fn init_buildings(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    r.game.grounds[g.0].x10_flags.b2 = false;
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// The crane's setup (map 1), `grFourside_801F3154`: its collision, its swing animation
/// stopped at the start, a wait, and the girder's height (ten below its model's) as where it
/// rests, is and is headed.
#[doc(alias = "grFourside_801F3154")]
pub fn init_crane(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let girder = nth_joint(r.game, gobj, 4);
    let m = map_id(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start(r.game, r.data, gobj, 0, 7, 0, 0.0, 0.0, true);
    r.game.grounds[g.0].u.fourside_crane().set_swung(false);
    let p = param(r.game);
    let wait = p.wait(r, p::CRANE_WAIT, p::CRANE_WAIT_ADD);
    let y = r.game.objs.jobjs[girder.expect("jobj")].translate.y;
    let v = r.game.grounds[g.0].u.fourside_crane();
    v.x4 = wait;
    v.x8 = y;
    v.x8 = mem(fp::fsubs(reg(v.x8), 10.0));
    v.x10 = v.x8;
    v.xC = v.x10;
    v.x0 = 0;
    let gp = &mut r.game.grounds[g.0];
    gp.x10_flags.b5 = true;
    gp.x11_flags.b012 = 1;
}

/// The crane's process (map 1), `grFourside_801F3274`. Waiting (state 0), it swings its
/// girder over (state 1, from the swing's start) or back (state 2, from frame 400) for 300
/// ticks; then (state 3) a wait before the girder moves (state 4) up or down by a random
/// distance from where it rests: down (state 5) when it sits above, up (state 6) otherwise, at
/// its speed, then bobs about its target (state 7), its pull doubling and turning each time it
/// passes, until the pull outgrows its starting speed, and it waits again. The girder's model
/// follows, and the collision.
#[doc(alias = "grFourside_801F3274")]
pub fn proc_crane(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let girder = nth_joint(r.game, gobj, 4);
    let p = param(r.game);
    let abs = |x: f64| if x < 0.0 { fp::fneg(x) } else { x };
    let v = r.game.grounds[g.0].u.fourside_crane();
    match v.x0 {
        0 => {
            if v.x4 <= 0 {
                let (frame, next) = if !v.swung() { (0.0, 1) } else { (400.0, 2) };
                anime::req_anim(r.game, gobj, 0, 7, frame, true);
                anime::set_rate(r.game, gobj, 0, 7, 1.0, true);
                let v = r.game.grounds[g.0].u.fourside_crane();
                v.x4 = 0;
                v.x0 = next;
            } else {
                v.x4 = v.x4.wrapping_sub(1);
            }
        }
        state @ (1 | 2) => {
            if v.x4 >= 300 {
                anime::set_rate(r.game, gobj, 0, 7, 0.0, true);
                let v = r.game.grounds[g.0].u.fourside_crane();
                v.set_swung(state == 1);
                v.x0 = 3;
            } else {
                v.x4 = v.x4.wrapping_add(1);
            }
        }
        3 => {
            let wait = p.wait(r, p::CRANE_IRON_WAIT, p::CRANE_IRON_WAIT_ADD);
            let v = r.game.grounds[g.0].u.fourside_crane();
            v.x4 = wait;
            v.x0 = 4;
        }
        4 => {
            if v.x4 <= 0 {
                v.x10 = v.x8;
                let down = reg(v.xC) > reg(v.x8);
                let (max, min) = if down {
                    (p::CRANE_IRON_DOWN_MAX, p::CRANE_IRON_DOWN_MIN)
                } else {
                    (p::CRANE_IRON_UP_MAX, p::CRANE_IRON_UP_MIN)
                };
                let span = abs(fp::fsubs(reg(p.f(r, max)), reg(p.f(r, min))));
                let n = fp::fctiwz(span);
                let k = if n != 0 { r.game.objs.rng.below(n) } else { 0 };
                let d = fp::frsp(f64::from(k));
                let from = abs(reg(p.f(r, min)));
                let v = r.game.grounds[g.0].u.fourside_crane();
                if down {
                    v.x10 = mem(fp::fsubs(reg(v.x10), fp::fadds(d, from)));
                    v.set_lowering(true);
                    crate::game::code::os_report();
                    r.game.grounds[g.0].u.fourside_crane().x0 = 5;
                } else {
                    v.x10 = mem(fp::fadds(reg(v.x10), fp::fadds(d, from)));
                    v.set_lowering(false);
                    crate::game::code::os_report();
                    r.game.grounds[g.0].u.fourside_crane().x0 = 6;
                }
            } else {
                v.x4 = v.x4.wrapping_sub(1);
            }
        }
        5 => {
            let spd = p.f(r, p::CRANE_IRON_SPD);
            let acl = p.f(r, p::CRANE_IRON_STOP_ACL);
            let v = r.game.grounds[g.0].u.fourside_crane();
            v.xC = mem(fp::fsubs(reg(v.xC), reg(spd)));
            if reg(v.xC) <= reg(v.x10) {
                v.xC = v.x10;
                v.x18 = spd;
                v.x14 = spd;
                v.x1C = acl;
                v.x0 = 7;
            }
        }
        6 => {
            let spd = p.f(r, p::CRANE_IRON_SPD);
            let acl = p.f(r, p::CRANE_IRON_STOP_ACL);
            let v = r.game.grounds[g.0].u.fourside_crane();
            v.xC = mem(fp::fadds(reg(v.xC), reg(spd)));
            if reg(v.xC) >= reg(v.x10) {
                v.x18 = spd;
                v.x14 = spd;
                v.x1C = acl;
                v.x0 = 7;
            }
        }
        7 => {
            let pull = reg(v.x1C);
            if abs(pull) > reg(v.x14) {
                v.xC = v.x10;
                let wait = p.wait(r, p::CRANE_WAIT, p::CRANE_WAIT_ADD);
                let v = r.game.grounds[g.0].u.fourside_crane();
                v.x4 = wait;
                v.x0 = 0;
            } else {
                v.x18 = mem(fp::fadds(reg(v.x18), pull));
                v.xC = mem(fp::fadds(reg(v.xC), reg(v.x18)));
                if reg(v.x1C) < 0.0 && reg(v.xC) < reg(v.x10) {
                    v.x1C = mem(fp::fmuls(reg(v.x1C), fp::fneg(2.0)));
                }
                if reg(v.x1C) > 0.0 && reg(v.xC) > reg(v.x10) {
                    v.x1C = mem(fp::fmuls(reg(v.x1C), fp::fneg(2.0)));
                }
            }
        }
        _ => {}
    }
    let y = r.game.grounds[g.0].u.fourside_crane().xC;
    let girder = girder.expect("jobj");
    jobj::set_srt(&mut r.game.objs.jobjs, girder, |o| o.translate.y = y);
    update_map_coll(r.game, r.data, gobj);
}

/// The UFO's setup (map 5), `grFourside_801F37FC`: its collision, a wait, no building and no
/// camera subject, hidden with its collision joint out of the search.
#[doc(alias = "grFourside_801F37FC")]
pub fn init_ufo(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    let wait = param(r.game).i(r, p::UFO_WAIT);
    let v = r.game.grounds[g.0].u.fourside_ufo();
    v.x4 = wait;
    v.x0 = 0;
    v.x1 = 0xFF;
    v.x8 = 0;
    v.x2 = 0;
    v.xC = None;
    v.x3 = 0;
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    r.game.geometry_mut().disable_joint(UFO_JOINT);
    r.game.grounds[g.0].x10_flags.b5 = true;
}

/// The UFO's process (map 5), `grFourside_801F3894`. Waiting (state 0), each time its wait
/// runs out it may come: never at first, at odds of 1 in `x46` while a fighter stands on the
/// roof, and 1 in `x48` once it has waited `ufo_challenge` times; coming, it picks a building
/// other than its last (the third only while the helicopter isn't about) and descends there
/// (state 1) with a camera subject framing it, its collision back. Down, it stays a random
/// while (state 2), then once its animation has rewound flies off (state 4) and hides after a
/// second. The roof's count clears every tick.
#[doc(alias = "grFourside_801F3894")]
pub fn proc_ufo(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let p = param(r.game);
    let m = map_id(r.game, gobj);
    let v = r.game.grounds[g.0].u.fourside_ufo();
    let (state, prev) = (v.x0, v.x1);
    match state {
        0 => {
            let timer = v.x4;
            if timer <= 0 {
                let mut prob = 0;
                if v.x8 != 0 {
                    prob = i32::from(p.h(r, p::X46));
                }
                let v = r.game.grounds[g.0].u.fourside_ufo();
                if i32::from(v.x2) >= i32::from(p.h(r, p::UFO_CHALLENGE)) {
                    prob = i32::from(p.h(r, p::X48));
                }
                if prob != 0 && r.game.objs.rng.below(prob) == 0 {
                    r.game.grounds[g.0].u.fourside_ufo().x2 = 0;
                    let building = loop {
                        let b = r.game.objs.rng.below(3);
                        if i32::from(prev) != b {
                            break b;
                        }
                    };
                    if building == 2 || !heli_about(r.game) {
                        r.game.grounds[g.0].u.fourside_ufo().x1 = building as u8;
                        let a = i32::from(r.game.grounds[g.0].u.fourside_ufo().x1) * 4;
                        anime::start_map(r.game, r.data, gobj, m, a);
                        update_coll_joint(r.game, r.data, UFO_JOINT);
                        r.game.geometry_mut().settle_vertices(UFO_JOINT);
                        jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
                        r.game.geometry_mut().enable_joint(UFO_JOINT);
                        let b = camera::alloc_box(&mut r.game.camera.boxes, camera::ACTIVE);
                        let v = r.game.grounds[g.0].u.fourside_ufo();
                        v.xC = Some(b);
                        v.x3 = 0;
                        v.x0 = 1;
                    }
                } else {
                    let wait = p.i(r, p::UFO_WAIT);
                    let v = r.game.grounds[g.0].u.fourside_ufo();
                    v.x4 = wait;
                    v.x2 = v.x2.wrapping_add(1);
                }
            } else {
                v.x4 = timer.wrapping_sub(1);
            }
        }
        1 => {
            if anime::finished(r.game, gobj, 0, 7) {
                let stay = p.wait(r, p::UFO_STAY_TIME, p::UFO_STAY_TIME_ADD);
                r.game.grounds[g.0].u.fourside_ufo().x4 = stay;
                anime::start_map(r.game, r.data, gobj, m, i32::from(prev) * 4 + 1);
                let b = r.game.grounds[g.0].u.fourside_ufo().xC;
                camera::free_box(
                    &mut r.game.camera.boxes,
                    b.expect("the UFO's camera subject"),
                );
                let v = r.game.grounds[g.0].u.fourside_ufo();
                v.xC = None;
                v.x0 = 2;
            }
            frame_ufo(r, gobj);
            let v = r.game.grounds[g.0].u.fourside_ufo();
            if v.x3 == 1 {
                let id = match v.x1 {
                    0 => 0x704E4,
                    1 => 0x704E0,
                    _ => 0x704E3,
                };
                r.stage_sound(id);
                let v = r.game.grounds[g.0].u.fourside_ufo();
                v.x3 = v.x3.wrapping_add(1);
            }
        }
        2 => {
            let timer = v.x4;
            if timer <= 0 {
                if anime::rewound(r.game, gobj, 0, 7) {
                    anime::start_map(r.game, r.data, gobj, m, i32::from(prev) * 4 + 2);
                    r.game.grounds[g.0].u.fourside_ufo().x0 = 4;
                }
            } else {
                v.x4 = timer.wrapping_sub(1);
            }
        }
        4 => {
            let timer = v.x4;
            if timer >= 0x3C {
                jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
                r.game.geometry_mut().disable_joint(UFO_JOINT);
                let wait = p.i(r, p::UFO_WAIT);
                let v = r.game.grounds[g.0].u.fourside_ufo();
                v.x4 = wait;
                v.x0 = 0;
            } else {
                v.x4 = timer.wrapping_add(1);
            }
        }
        _ => {}
    }
    update_map_coll(r.game, r.data, gobj);
    r.game.grounds[g.0].u.fourside_ufo().x8 = 0;
}

/// The UFO's camera subject, `grFourside_801F3B70`: once the UFO is low enough (its part 1
/// below the camera's top plus `ufo_cs_offs`), the subject frames it, and its arrival sound is
/// due (`x3` from 0 to 1).
#[doc(alias = "grFourside_801F3B70")]
pub fn frame_ufo(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = nth_joint(r.game, gobj, 1);
    let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, None);
    let Some(b) = r.game.grounds[g.0].u.fourside_ufo().xC else {
        return;
    };
    let top = super::stage::cam_top(r.game);
    let offs = param(r.game).f(r, p::UFO_CS_OFFS);
    if reg(pos.y) > fp::fadds(reg(offs), reg(top)) {
        return;
    }
    let v = r.game.grounds[g.0].u.fourside_ufo();
    if v.x3 == 0 {
        v.x3 = 1;
    }
    let cam = &mut r.game.camera.boxes[b];
    cam.pos = pos;
    cam.bone_pos = pos;
    cam.target_ext.h.x = -50.0;
    cam.target_ext.h.y = 50.0;
    cam.target_ext.v.x = 15.0;
    cam.target_ext.v.y = -15.0;
}

/// The helicopter's setup (map 3), `grFourside_801F3C40`: hidden, a wait.
#[doc(alias = "grFourside_801F3C40")]
pub fn init_heli(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    r.game.grounds[g.0].x11_flags.b012 = 1;
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    r.game.grounds[g.0].u.fourside2().x0 = 0;
    let p = param(r.game);
    let wait = p.wait(r, p::HELI_WAIT, p::HELI_WAIT_ADD);
    let v = r.game.grounds[g.0].u.fourside2();
    v.x4 = wait;
    v.x1 = 0;
}

/// Whether the UFO is out: neither waiting nor over the third building.
fn ufo_out(game: &mut Game) -> bool {
    let u = map_gobj(game, MAP_UFO).expect("Ground_GetMapGObj(5)");
    let ug = ground_of(game, u);
    let v = game.grounds[ug.0].u.fourside_ufo();
    !(v.x1 == 2 || v.x0 == 0)
}

/// The helicopter's process (map 3), `grFourside_801F3CC8`. Once its wait is over and the UFO
/// isn't out, it flies in (state 1, shown; its sound once its part 2 is low enough), hovers
/// (state 2) until its wait is over and the UFO isn't out again, then leaves (state 3) and
/// hides with a new wait.
#[doc(alias = "grFourside_801F3CC8")]
pub fn proc_heli(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let heli = nth_joint(r.game, gobj, 2);
    let m = map_id(r.game, gobj);
    match r.game.grounds[g.0].u.fourside2().x0 {
        0 => {
            if r.game.grounds[g.0].u.fourside2().x4 <= 0 {
                if !ufo_out(r.game) {
                    jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
                    anime::start_map(r.game, r.data, gobj, m, 0);
                    let v = r.game.grounds[g.0].u.fourside2();
                    v.x1 = 0;
                    v.x0 = 1;
                }
            } else {
                let v = r.game.grounds[g.0].u.fourside2();
                v.x4 = v.x4.wrapping_sub(1);
            }
        }
        1 => {
            if r.game.grounds[g.0].u.fourside2().x1 == 0 {
                let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, heli, None);
                let top = super::stage::cam_top(r.game);
                if reg(pos.y) <= fp::fadds(50.0, reg(top)) {
                    r.game.grounds[g.0].u.fourside2().x1 = 1;
                    r.stage_sound(0x704E1);
                }
            }
            if anime::finished(r.game, gobj, 0, 7) {
                r.game.grounds[g.0].u.fourside2().x0 = 2;
            }
        }
        2 => {
            if r.game.grounds[g.0].u.fourside2().x4 <= 0 {
                if !ufo_out(r.game) {
                    anime::start_map(r.game, r.data, gobj, m, 2);
                    r.game.grounds[g.0].u.fourside2().x0 = 3;
                    r.stage_sound(0x704E2);
                    r.game.grounds[g.0].u.fourside2().x4 = 0;
                }
            } else {
                let v = r.game.grounds[g.0].u.fourside2();
                v.x4 = v.x4.wrapping_sub(1);
            }
        }
        3 => {
            if anime::finished(r.game, gobj, 0, 7) {
                jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
                r.game.grounds[g.0].u.fourside2().x0 = 0;
                let p = param(r.game);
                let wait = p.wait(r, p::HELI_WAIT, p::HELI_WAIT_ADD);
                r.game.grounds[g.0].u.fourside2().x4 = wait;
            }
            let v = r.game.grounds[g.0].u.fourside2();
            v.x4 = v.x4.wrapping_add(1);
        }
        _ => {}
    }
}

/// Whether the helicopter is about, `grFourside_801F3F10`: flying in and not yet low, or
/// leaving within its first 700 ticks.
#[doc(alias = "grFourside_801F3F10")]
pub fn heli_about(game: &mut Game) -> bool {
    let h = map_gobj(game, 3).expect("Ground_GetMapGObj(3)");
    let hg = ground_of(game, h);
    let v = game.grounds[hg.0].u.fourside2();
    (v.x0 == 1 && v.x1 == 0) || (v.x0 == 3 && v.x4 <= 700)
}

/// The city's render callback, `fn_801F3F74`: in the display pass, unless the camera looks
/// almost straight up or down, the skyline's three layers turn about y by 0.2, 0.4 and 0.7 of
/// the camera's heading; then drawn as any stage object is.
#[doc(alias = "fn_801F3F74")]
pub fn render_city(r: &mut Running, gobj: GObjId, pass: i32) {
    if pass == 1 {
        let g = ground_of(r.game, gobj);
        let cur = crate::hsd::cobj::current(&r.game.objs.cobjs);
        let eye = crate::hsd::cobj::eye_vector(&mut r.game.objs, r.data, cur)
            .unwrap_or(crate::hsd::cobj::NO_EYE_VECTOR);
        let y = reg(eye.y);
        let abs_y = if y < 0.0 { fp::fneg(y) } else { y };
        if abs_y < reg(0.99f32) {
            let angle = reg(crate::math::atan2(
                mem(fp::fneg(reg(eye.x))),
                mem(fp::fneg(reg(eye.z))),
            ));
            let v = r.game.grounds[g.0].u.fourside();
            let layers: [(Option<JObjId>, f32); 3] = [(v.x0, 0.2), (v.x4, 0.4), (v.x8, 0.7)];
            for (j, k) in layers {
                if let Some(j) = j {
                    let a = mem(fp::fmuls(reg(k), angle));
                    jobj::set_rotation_y(&mut r.game.objs.jobjs, j, a);
                }
            }
        }
    }
    r.render_stage_object(gobj, pass);
}

/// The stage's shadow check, `grFourside_801F41E8`: a fighter casts on a joint below its feet.
#[doc(alias = "grFourside_801F41E8")]
pub fn check_shadow_render(
    game: &mut Game,
    data: &melee_data::archive::Archives,
    pos: Vec3,
    j: Option<JObjId>,
) -> bool {
    let b = crate::lb::joint::world_position(&mut game.objs, data, j, None);
    reg(pos.y) > reg(b.y)
}
