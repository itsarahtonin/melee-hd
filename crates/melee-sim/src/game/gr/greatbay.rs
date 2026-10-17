//! Great Bay, `melee/gr/grgreatbay.c`: the island the fighters stand on (map 2) with its floating
//! platform, the turtle (map 1), Tingle (map 10, riding the balloon item `it::kinds::tincle`),
//! the moon and the four giants (maps 9, 5 to 8), the background and the water in front (maps 3
//! and 4), and map 0.
//!
//! The turtle surfaces now and then, facing a random way, to be stood on, and dives again. The
//! platform tips and sinks where fighters land on it. Tingle floats over the stage; stood on, his
//! balloon sinks, and hit, it pops and he falls. A fighter dropping into the sea splashes.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::handles::CodeRef;
use super::super::it::kinds::tincle;
use super::super::state::{Game, Running};
use super::data::{GREATBAY_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, map_gobj, nth_joint, set_guide_points, update_map_coll};
use super::{anime, grlib, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

/// `fn_801F4520`, the island's start hook: registers the sea as a fighter device.
pub const START_HOOK: u32 = 0x801F_4520;
/// `grGreatBay_801F6708`, the sea's device callback: a fighter falling in splashes.
pub const SEA_DEVICE: u32 = 0x801F_6708;
/// `grGreatBay_801F60C4`, the platform's collision joint's callback.
pub const PLATFORM_TOUCHED: u32 = 0x801F_60C4;
/// `grGreatBay_801F5914`, Tingle's balloon's collision joint's callback.
pub const BALLOON_TOUCHED: u32 = 0x801F_5914;

/// Tingle's map id.
const MAP_TINGLE: i32 = 10;
/// The balloon's collision joint.
const BALLOON_JOINT: i32 = 5;

/// The fields of `grGb_StageAttr`, by offset.
mod p {
    pub const MOON_FALL_WAIT_A: u32 = 0;
    pub const MOON_FALL_WAIT_B: u32 = 2;
    pub const LANDING_RATE: u32 = 4;
    pub const SLANT_MUL: u32 = 8;
    pub const SLANT_ADD: u32 = 0xC;
    pub const SLANT_LIMIT: u32 = 0x10;
    pub const SLANT_RATE: u32 = 0x14;
    pub const SLANT_REB_RATE: u32 = 0x18;
    pub const SLIDE_MUL: u32 = 0x1C;
    pub const SLIDE_ADD: u32 = 0x20;
    pub const SLIDE_LIMIT: u32 = 0x24;
    pub const SLIDE_RATE: u32 = 0x28;
    pub const SLIDE_REB_RATE: u32 = 0x2C;
    pub const DOWN_MUL: u32 = 0x30;
    pub const DOWN_ADD: u32 = 0x34;
    pub const DOWN_LIMIT: u32 = 0x38;
    pub const DOWN_UP_RATE: u32 = 0x3C;
    pub const DOWN_DOWN_RATE: u32 = 0x40;
    pub const KAME_WAIT_A: u32 = 0x44;
    pub const KAME_WAIT_B: u32 = 0x46;
    pub const KAME_REBIRTH_A: u32 = 0x48;
    pub const KAME_REBIRTH_B: u32 = 0x4A;
    pub const KAME_X: u32 = 0x4C;
    pub const KAME_Y: u32 = 0x50;
    pub const KAME_X_OFFSET_INIT: u32 = 0x54;
    pub const KAME_X_LR_A: u32 = 0x58;
    pub const KAME_X_LR_B: u32 = 0x5C;
    pub const KAME_X_FB_A: u32 = 0x60;
    pub const KAME_X_FB_B: u32 = 0x64;
    pub const KAME_SCALE: u32 = 0x68;
    pub const KAME_UD_SCALE: u32 = 0x6C;
    pub const KAME_DIR_PROB: u32 = 0x70;
    pub const KAME_ITEM_PROB: u32 = 0x78;
    pub const ITEMS: u32 = 0x7C;
}

#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn h(self, r: &Running, off: u32) -> i16 {
        r.data.u16(self.0.at(off)) as i16
    }
    fn f(self, r: &Running, off: u32) -> f32 {
        r.data.f32(self.0.at(off))
    }
}

fn param(game: &Game) -> Param {
    Param(
        game.gr
            .greatbay_yakumono
            .expect("grgreatbay.c: grGb_804D69E0"),
    )
}

/// A stage object's process or callback, by address: Great Bay's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x801F_4438 | 0x801F_443C | 0x801F_44A8 | 0x801F_44AC | 0x801F_4518 | 0x801F_451C
        | 0x801F_4690 | 0x801F_545C | 0x801F_5988 | 0x801F_59F8 | 0x801F_59FC | 0x801F_5D48 => {
            empty(r, gobj)
        }
        0x801F_4404 => init_main(r, gobj),
        0x801F_4440 => init_back(r, gobj),
        0x801F_44B0 => init_water(r, gobj),
        START_HOOK => start_sea(r, gobj),
        0x801F_454C => init_island(r, gobj),
        0x801F_4658 => proc_island(r, gobj),
        0x801F_4694 => init_turtle(r, gobj),
        0x801F_499C => proc_turtle(r, gobj),
        0x801F_5460 => init_tingle(r, gobj),
        0x801F_5600 => proc_tingle(r, gobj),
        0x801F_598C => init_giant(r, gobj),
        0x801F_5A00 => init_moon(r, gobj),
        0x801F_5AF8 => proc_moon(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x801F_4240 => on_init(r),
        0x801F_42D0 => on_load(r),
        0x801F_42D4 => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grGreatBay_801F4438")]
#[doc(alias = "grGreatBay_801F443C")]
#[doc(alias = "grGreatBay_801F44A8")]
#[doc(alias = "grGreatBay_801F44AC")]
#[doc(alias = "grGreatBay_801F4518")]
#[doc(alias = "grGreatBay_801F451C")]
#[doc(alias = "grGreatBay_801F4690")]
#[doc(alias = "grGreatBay_801F545C")]
#[doc(alias = "grGreatBay_801F5988")]
#[doc(alias = "grGreatBay_801F59F8")]
#[doc(alias = "grGreatBay_801F59FC")]
#[doc(alias = "grGreatBay_801F5D48")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `grGreatBay_801F4300`: as the other
/// stages do, but Tingle's process runs at priority 5.
#[doc(alias = "grGreatBay_801F4300")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    let cb = callbacks(map_id);
    let gobj = setup::stage_gobj(r, map_id, frame)?;
    let g = ground_of(r.game, gobj);
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.xC_callback = None;
    r.game.world.setup_gx_link(gobj, CodeRef(0x801C_5DB0), 3, 0);
    if cb.callback3 != 0 {
        r.game.grounds[g.0].x1C_callback = Some(CodeRef(cb.callback3));
    }
    if cb.on_init != 0 {
        super::super::code::run_proc(r, gobj, CodeRef(cb.on_init));
    }
    if cb.gobj_proc != 0 {
        let pri = if map_id == MAP_TINGLE { 5 } else { 4 };
        r.game.world.setup_proc(gobj, CodeRef(cb.gobj_proc), pri);
    }
    Some(gobj)
}

/// The stage's objects' setup functions and processes, by map id (`grGb_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &GREATBAY_CALLBACKS[map_id as usize]
}

/// Great Bay's setup, `grGreatBay_801F4240`: its parameters, maps 0, 3, 4, 2, 1, 9 (which makes
/// the giants) and 10, camera limits and blast zones.
#[doc(alias = "grGreatBay_801F4240")]
pub fn on_init(r: &mut Running) {
    r.game.gr.greatbay_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [0, 3, 4, 2, 1, 9, MAP_TINGLE] {
        create(r, map_id, setup::SETUP_FRAME);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
}

/// `grGreatBay_801F423C` does nothing (only a demo runs it).
#[doc(alias = "grGreatBay_801F423C")]
pub fn on_demo_init(_r: &mut Running) {}

/// `grGreatBay_801F42D0` does nothing.
#[doc(alias = "grGreatBay_801F42D0")]
pub fn on_load(_r: &mut Running) {}

/// Great Bay's start, `grGreatBay_801F42D4`: the enemy spawner, without enemies.
#[doc(alias = "grGreatBay_801F42D4")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// Map 0's setup, `stageGObj0_OnInit`: its animation.
#[doc(alias = "stageGObj0_OnInit@801F4404")]
pub fn init_main(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The background's setup (map 3), `grGreatBay_801F4440`: its animation, looping.
#[doc(alias = "grGreatBay_801F4440")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    anime::set_loop(r.game, gobj, 0, 7, true);
    r.game.grounds[g.0].x11_flags.b012 = 1;
}

/// The water in front's setup (map 4), `grGreatBay_801F44B0`: its animation, looping, drawn in
/// the foreground layer.
#[doc(alias = "grGreatBay_801F44B0")]
pub fn init_water(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    anime::set_loop(r.game, gobj, 0, 7, true);
    r.game.grounds[g.0].x11_flags.b012 = 3;
}

/// The island's start hook, `fn_801F4520`: the sea as a fighter device of type 6
/// (`ftCo_800C07F8`).
#[doc(alias = "fn_801F4520")]
pub fn start_sea(r: &mut Running, gobj: GObjId) {
    r.game
        .gr
        .devices
        .register_coll(gobj, 6, CodeRef(SEA_DEVICE));
}

/// The island's setup (map 2), `grGreatBay_801F454C`: its collision and animation, the guide
/// points, the turtle's joint out of the search, the floating platform, the start hook, and it
/// receives shadows.
#[doc(alias = "grGreatBay_801F454C")]
pub fn init_island(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.x11_flags.b012 = 1;
    gp.xC_callback = None;
    gp.x10_flags.b5 = true;
    let n = |r: &Running, k: i32| nth_joint(r.game, gobj, k);
    let points = [n(r, 4), n(r, 1), n(r, 3), n(r, 2), n(r, 6), n(r, 5)];
    set_guide_points(r.game, r.data, points);
    r.game.geometry_mut().disable_joint(1);
    init_platform(r, gobj);
    setup::add_start_hook(r.game, gobj, CodeRef(START_HOOK));
}

/// The island's process, `grGreatBay_801F4658`: the platform, its collision following the
/// model, and the wind sources.
#[doc(alias = "grGreatBay_801F4658")]
pub fn proc_island(r: &mut Running, gobj: GObjId) {
    proc_platform(r, gobj);
    update_map_coll(r.game, r.data, gobj);
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
}

/// The turtle's setup (map 1), `grGreatBay_801F4694`: its collision and animation, scaled and
/// placed by the parameters, its shell's height noted, facing the stage, waiting a random
/// while.
#[doc(alias = "grGreatBay_801F4694")]
pub fn init_turtle(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("jobj");
    setup::init_map_coll(r.game, r.data, Some(j), m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let p = param(r.game);
    let k = reg(p.f(r, p::KAME_SCALE));
    let s = r.game.objs.jobjs[j].scale;
    let s = Vec3::new(
        mem(fp::fmuls(reg(s.x), k)),
        mem(fp::fmuls(reg(s.y), k)),
        mem(fp::fmuls(reg(s.z), k)),
    );
    jobj::set_scale(&mut r.game.objs.jobjs, j, s);
    let x = mem(fp::fadds(
        reg(p.f(r, p::KAME_X)),
        reg(p.f(r, p::KAME_X_OFFSET_INIT)),
    ));
    let y = p.f(r, p::KAME_Y);
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.x = x);
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = y);
    let shell = nth_joint(r.game, gobj, 2).expect("jobj");
    let shell_y = r.game.objs.jobjs[shell].translate.y;
    let (b, a) = (
        i32::from(p.h(r, p::KAME_WAIT_B)),
        i32::from(p.h(r, p::KAME_WAIT_A)),
    );
    let v = r.game.grounds[g.0].u.greatbay();
    v.xCC = shell_y;
    v.xC4 = 0;
    v.set_dir(0);
    let wait = grlib::rand_range(&mut r.game.objs.rng, b, a) as i16;
    let v = r.game.grounds[g.0].u.greatbay();
    v.xC6 = wait;
    v.set_item_done(true);
    v.xC8 = None;
    let gp = &mut r.game.grounds[g.0];
    gp.x10_flags.b5 = true;
    gp.x11_flags.b012 = 1;
}

/// The turtle's process (map 1), `grGreatBay_801F499C`. Under water (state 0), once its wait
/// is over and its animation has rewound it surfaces... no: it dives (animation 1, state 1:
/// its joints kept out of the islands, a splash 90 ticks in), then stays down (state 3) a
/// random while; then it surfaces (state 2) at a place by the way it faces (a new random
/// direction: 0 sideways, 1 and 3 front and back), splashing, with an item now and then, and
/// once up waits (state 0) again. Its shell's height scales by the parameters, its head turns
/// the way it faces, and its collision follows.
#[doc(alias = "grGreatBay_801F499C")]
pub fn proc_turtle(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("grgreatbay.c: jobj");
    let shell = nth_joint(r.game, gobj, 2).expect("grgreatbay.c: trans_jobj");
    let head = nth_joint(r.game, gobj, 3);
    let p = param(r.game);
    match r.game.grounds[g.0].u.greatbay().xC4 {
        0 => {
            if r.game.grounds[g.0].u.greatbay().xC6 < 0 {
                if anime::rewound(r.game, gobj, 0, 7) {
                    let v = r.game.grounds[g.0].u.greatbay();
                    v.xC4 = 1;
                    anime::start_map(r.game, r.data, gobj, m, 1);
                    r.game.grounds[g.0].u.greatbay().xC6 = 0;
                    let geo = r.game.geometry_mut();
                    for k in [2, 3, 4, 1] {
                        geo.forbid_islands(k);
                    }
                }
            } else {
                let v = r.game.grounds[g.0].u.greatbay();
                v.xC6 = v.xC6.wrapping_sub(1);
            }
        }
        1 => {
            let v = r.game.grounds[g.0].u.greatbay();
            let t = v.xC6;
            v.xC6 = t.wrapping_add(1);
            if t == 90 {
                r.stage_sound(0x57E42);
                let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, Some(j), None);
                let generator = grlib::generator_at(r.game, r.data, 0x7531, 0x1E, pos);
                r.game.grounds[g.0].u.greatbay().xC8 = generator;
            }
            if anime::finished(r.game, gobj, 0, 7) {
                r.game.grounds[g.0].u.greatbay().xC4 = 3;
                let (b, a) = (
                    i32::from(p.h(r, p::KAME_REBIRTH_B)),
                    i32::from(p.h(r, p::KAME_REBIRTH_A)),
                );
                let wait = grlib::rand_range(&mut r.game.objs.rng, b, a) as i16;
                r.game.grounds[g.0].u.greatbay().xC6 = wait;
                anime::set_rate(r.game, gobj, 0, 7, 0.0, true);
                jobj::set_flags_all(&mut r.game.objs.jobjs, Some(j), jobj::HIDDEN);
                if let Some(generator) = r.game.grounds[g.0].u.greatbay().xC8.take() {
                    grlib::kill_generator_and_particles(r.game, r.data, generator);
                }
            }
        }
        2 => {
            jobj::clear_flags_all(&mut r.game.objs.jobjs, Some(j), jobj::HIDDEN);
            let v = r.game.grounds[g.0].u.greatbay();
            v.xC6 = v.xC6.wrapping_add(1);
            let t = v.xC6;
            if t == 60 {
                r.stage_sound(0x57E41);
                let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, Some(j), None);
                let generator = grlib::generator_at(r.game, r.data, 0x7531, 0x1E, pos);
                r.game.grounds[g.0].u.greatbay().xC8 = generator;
            }
            let t = r.game.grounds[g.0].u.greatbay().xC6;
            if t == 60 || t == 120 {
                let fin = nth_joint(r.game, gobj, 34);
                if fin.is_some() {
                    grlib::generator_oriented_on(r.game, r.data, 0x7533, 0x1E, fin);
                }
            }
            let t = r.game.grounds[g.0].u.greatbay().xC6;
            if t == 90 || t == 150 {
                let fin = nth_joint(r.game, gobj, 38);
                if fin.is_some() {
                    grlib::generator_oriented_on(r.game, r.data, 0x7533, 0x1E, fin);
                }
            }
            if anime::finished(r.game, gobj, 0, 7) {
                r.game.grounds[g.0].u.greatbay().xC4 = 0;
                let (b, a) = (
                    i32::from(p.h(r, p::KAME_WAIT_B)),
                    i32::from(p.h(r, p::KAME_WAIT_A)),
                );
                let wait = grlib::rand_range(&mut r.game.objs.rng, b, a) as i16;
                r.game.grounds[g.0].u.greatbay().xC6 = wait;
                anime::start_map(r.game, r.data, gobj, m, 0);
                let geo = r.game.geometry_mut();
                for k in [2, 3, 4, 1] {
                    geo.allow_islands(k);
                }
                if let Some(generator) = r.game.grounds[g.0].u.greatbay().xC8.take() {
                    grlib::kill_generator_and_particles(r.game, r.data, generator);
                }
            }
        }
        3 => {
            let v = r.game.grounds[g.0].u.greatbay();
            let t = v.xC6;
            v.xC6 = t.wrapping_sub(1);
            if t < 0 {
                v.xC4 = 2;
                anime::start_map(r.game, r.data, gobj, m, 2);
                let v = r.game.grounds[g.0].u.greatbay();
                v.xC6 = 0;
                v.set_item_done(false);
                let dir = v.dir();
                let dir = turtle_dir(r, dir as i32) as u8;
                r.game.grounds[g.0].u.greatbay().set_dir(dir);
                let x = |r: &mut Running, b: u32, a: u32| {
                    let (b, a) = (reg(p.f(r, b)), reg(p.f(r, a)));
                    let off = grlib::rand_range(&mut r.game.objs.rng, fp::fctiwz(b), fp::fctiwz(a));
                    mem(fp::fadds(reg(p.f(r, p::KAME_X)), fp::frsp(f64::from(off))))
                };
                match dir {
                    1 | 3 => {
                        let x = x(r, p::KAME_X_FB_B, p::KAME_X_FB_A);
                        jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.x = x);
                        let geo = r.game.geometry_mut();
                        for k in [2, 3, 4] {
                            geo.disable_joint(k);
                        }
                        geo.enable_joint(1);
                    }
                    2 => panic!("grgreatbay.c:652: 0"),
                    0 => {
                        let x = x(r, p::KAME_X_LR_B, p::KAME_X_LR_A);
                        jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.x = x);
                        let geo = r.game.geometry_mut();
                        for k in [2, 3, 4] {
                            geo.enable_joint(k);
                        }
                        geo.disable_joint(1);
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    let y = r.game.objs.jobjs[shell].translate.y;
    if reg(r.game.grounds[g.0].u.greatbay().xCC) != reg(y) {
        let y = mem(fp::fmuls(reg(y), reg(p.f(r, p::KAME_UD_SCALE))));
        jobj::set_srt(&mut r.game.objs.jobjs, shell, |o| o.translate.y = y);
        r.game.grounds[g.0].u.greatbay().xCC = y;
    }
    match r.game.grounds[g.0].u.greatbay().dir() {
        1 => {
            let h = head.expect("jobj");
            jobj::set_rotation_y(&mut r.game.objs.jobjs, h, std::f32::consts::FRAC_PI_2);
        }
        2 => panic!("grgreatbay.c:692: 0"),
        0 => jobj::set_rotation_y(&mut r.game.objs.jobjs, shell, 0.0),
        3 => {
            let h = head.expect("jobj");
            jobj::set_rotation_y(&mut r.game.objs.jobjs, h, -std::f32::consts::FRAC_PI_2);
        }
        _ => {}
    }
    update_map_coll(r.game, r.data, gobj);
    follow_vertices(r, gobj);
    r.game.geometry_mut().update_dynamics_and_islands(2);
    let v = r.game.grounds[g.0].u.greatbay();
    if v.xC4 == 2 && !v.item_done() {
        let done = turtle_item(r, gobj);
        r.game.grounds[g.0].u.greatbay().set_item_done(done);
    }
}

/// Tingle's setup (map 10), `grGreatBay_801F5460`: his balloon's collision (with its callback,
/// in the search) far off at (1000, 1000), and the balloon item made.
#[doc(alias = "grGreatBay_801F5460")]
pub fn init_tingle(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("jobj");
    setup::init_map_coll(r.game, r.data, Some(j), m);
    r.game.grounds[g.0].xC_callback = None;
    r.game.geometry_mut().set_ledge_callback(
        BALLOON_JOINT,
        Some(super::CollUser::Ground(g)),
        Some(CodeRef(BALLOON_TOUCHED)),
    );
    r.game.geometry_mut().enable_joint(BALLOON_JOINT);
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.x = 1000.0);
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = 1000.0);
    update_map_coll(r.game, r.data, gobj);
    r.game.geometry_mut().settle_vertices(BALLOON_JOINT);
    let item = tincle::create(r, gobj);
    reset_tingle(r, g, item);
}

/// Tingle's variables after a new balloon.
fn reset_tingle(r: &mut Running, g: super::super::handles::GroundId, item: Option<GObjId>) {
    let v = r.game.grounds[g.0].u.greatbay4();
    v.xF0 = item;
    v.xC4 = 0;
    v.xC8 = 0;
    v.xD0 = 0;
    v.xCC = 0;
    v.xD8 = 0;
    v.xDC = 0;
    v.xE0 = 0.0;
}

/// Tingle's process (map 10), `grGreatBay_801F5600`: follows the balloon item's state and place
/// (a new balloon once it falls below -50), hands it the weight stood on it, presses it down
/// while fighters stand on it, pops it with a sound and an effect, splashes when it lands in
/// the sea, and moves his collision with it.
#[doc(alias = "grGreatBay_801F5600")]
pub fn proc_tingle(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("jobj");
    let item = r.game.grounds[g.0].u.greatbay4().xF0;
    let item = item.expect("Tingle's balloon");
    let (state, pos) = tincle::state_and_place(r.game, item);
    let v = r.game.grounds[g.0].u.greatbay4();
    v.xC4 = state;
    v.xE4 = pos;
    if reg(pos.y) < fp::fneg(50.0) {
        tincle::destroy(r, item);
        r.game.geometry_mut().settle_vertices(BALLOON_JOINT);
        let new = tincle::create(r, gobj);
        reset_tingle(r, g, new);
        return;
    }
    let weight = r.game.grounds[g.0].u.greatbay4().xE0;
    tincle::set_weight(r.game, item, weight);
    let v = r.game.grounds[g.0].u.greatbay4();
    match v.xC4 {
        1 => {
            r.game.geometry_mut().enable_joint(BALLOON_JOINT);
            update_map_coll(r.game, r.data, gobj);
        }
        2 | 5 => {
            if v.xD8 != 0 && v.xD4 == 0 {
                tincle::press(r, item);
            }
        }
        4 => {
            if v.xD8 == 0 {
                tincle::release(r, item);
            } else if v.xDC != v.xD8 {
                tincle::press(r, item);
            }
        }
        6 | 7 => {
            let (lit, mut at) = (v.xCC, v.xE4);
            if lit == 0 && tincle::timer(r.game, item) == 0 {
                at.y = mem(fp::fadds(reg(at.y), 10.0));
                grlib::generator_at(r.game, r.data, 0x7534, 0x1E, at);
                r.stage_sound(0x57E4A);
                r.game.grounds[g.0].u.greatbay4().xCC = 1;
            }
            r.game.geometry_mut().disable_joint(BALLOON_JOINT);
        }
        8 => {
            if v.xD0 == 0 && reg(v.xE4.y) <= 0.0 {
                v.xD0 = 1;
                let at = v.xE4;
                grlib::generator_at(r.game, r.data, 0x7537, 0x1E, at);
                r.stage_sound(0x57E48);
            }
        }
        9 => {
            if v.xC8 != v.xC4 {
                r.stage_sound(0x57E47);
            }
        }
        12 => {
            if v.xC8 != v.xC4 {
                v.xCC = 0;
                r.game.geometry_mut().enable_joint(BALLOON_JOINT);
            }
            if r.game.grounds[g.0].u.greatbay4().xD8 != 0 {
                tincle::press(r, item);
            }
        }
        _ => {}
    }
    let v = r.game.grounds[g.0].u.greatbay4();
    v.xC8 = v.xC4;
    let at = v.xE4;
    jobj::set_translate(&mut r.game.objs.jobjs, j, at);
    update_map_coll(r.game, r.data, gobj);
    let v = r.game.grounds[g.0].u.greatbay4();
    v.xDC = v.xD8;
    v.xD8 = 0;
    v.xE0 = 0.0;
    update_map_coll(r.game, r.data, gobj);
}

/// The balloon's collision joint's callback, `grGreatBay_801F5914`: a fighter, item or other
/// object (collision kinds 1 to 3) on it notes whether it hangs from a ledge, counts, and adds
/// its weight (`x50`) in hundreds.
#[doc(alias = "grGreatBay_801F5914")]
pub fn balloon_touched(
    gp: &mut super::super::structs::Ground,
    coll: &super::super::structs::CollData,
    x50: i32,
) {
    let kind = u32::from(coll.x34_flags.b1234);
    if kind == 1 || kind == 2 || kind == 3 {
        let v = gp.u.greatbay4();
        v.xD4 =
            coll.env_flags & (crate::mp::coll::LEFT_LEDGE_GRAB | crate::mp::coll::RIGHT_LEDGE_GRAB);
        v.xD8 = v.xD8.wrapping_add(1);
        v.xE0 = mem(fp::fadds(
            reg(v.xE0),
            fp::fdivs(fp::frsp(f64::from(x50)), 100.0),
        ));
    }
}

/// A giant's setup (maps 5 to 8), `grGreatBay_801F598C`: its animation, stopped, in the
/// background layer.
#[doc(alias = "grGreatBay_801F598C")]
pub fn init_giant(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    anime::set_rate(r.game, gobj, 0, 7, 0.0, true);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// The moon's setup (map 9), `grGreatBay_801F5A00`: its animation, stopped, the four giants
/// made, and a wait before it falls.
#[doc(alias = "grGreatBay_801F5A00")]
pub fn init_moon(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    anime::set_rate(r.game, gobj, 0, 7, 0.0, true);
    for (k, map) in (5..=8).enumerate() {
        let giant = create(r, map, MOON_FRAME);
        r.game.grounds[g.0].u.greatbay2().gobjs[k] = giant;
    }
    let p = param(r.game);
    let (b, a) = (
        i32::from(p.h(r, p::MOON_FALL_WAIT_B)),
        i32::from(p.h(r, p::MOON_FALL_WAIT_A)),
    );
    let wait = grlib::rand_range(&mut r.game.objs.rng, b, a) as i16;
    r.game.grounds[g.0].u.greatbay2().x10 = wait;
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// Where the moon's setup puts the giants' scale joints' descriptors on the stack: below the
/// stage's setup's by the create function's frame and the moon's setup's (0x20 each).
pub const MOON_FRAME: u32 = setup::SETUP_FRAME - 0x40;

/// The moon's process (map 9), `grGreatBay_801F5AF8`: its wait counts down, then it and the
/// giants play their animations; once it has played (any animation rewound), they stop and a
/// new wait starts. Its flags note the animation under way.
#[doc(alias = "grGreatBay_801F5AF8")]
pub fn proc_moon(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let v = r.game.grounds[g.0].u.greatbay2();
    if v.x10 > 0 {
        v.x10 -= 1;
        if v.x10 == 0 {
            anime::start_map(r.game, r.data, gobj, m, 0);
            anime::set_rate(r.game, gobj, 0, 7, 1.0, true);
            for k in 0..4 {
                let Some(giant) = r.game.grounds[g.0].u.greatbay2().gobjs[k] else {
                    continue;
                };
                let gm = map_id(r.game, giant);
                anime::start_map(r.game, r.data, giant, gm, 0);
                anime::set_rate(r.game, giant, 0, 7, 1.0, true);
            }
            r.game.grounds[g.0].u.greatbay2().x12 &= !0xF0;
        }
    } else {
        let frame = super::ground::anim_frame(r.game, j);
        let v = r.game.grounds[g.0].u.greatbay2();
        for bit in [0x80u8, 0x40, 0x20, 0x10] {
            if v.x12 & bit == 0 && reg(frame) > 0.0 {
                v.x12 |= bit;
            }
        }
        if crate::lb::joint::any_rewound(&r.game.objs, j) {
            anime::set_rate(r.game, gobj, 0, 7, 0.0, true);
            for k in 0..4 {
                if let Some(giant) = r.game.grounds[g.0].u.greatbay2().gobjs[k] {
                    anime::set_rate(r.game, giant, 0, 7, 0.0, true);
                }
            }
            let p = param(r.game);
            let (b, a) = (
                i32::from(p.h(r, p::MOON_FALL_WAIT_B)),
                i32::from(p.h(r, p::MOON_FALL_WAIT_A)),
            );
            let wait = grlib::rand_range(&mut r.game.objs.rng, b, a) as i16;
            r.game.grounds[g.0].u.greatbay2().x10 = wait;
        }
    }
}

/// The floating platform's setup, `grGreatBay_801F5D4C`: its collision joint's callback, its
/// model joint and resting place, and nothing pushing it.
#[doc(alias = "grGreatBay_801F5D4C")]
pub fn init_platform(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    r.game.geometry_mut().set_ledge_callback(
        0,
        Some(super::CollUser::Ground(g)),
        Some(CodeRef(PLATFORM_TOUCHED)),
    );
    let j = nth_joint(r.game, gobj, 49);
    let Some(jj) = j else {
        panic!("grgreatbay.c:1132: 0");
    };
    let t = r.game.objs.jobjs[jj].translate;
    let v = r.game.grounds[g.0].u.greatbay3();
    v.jobj = j;
    v.translation.x = t.x;
    v.translation.y = t.y;
    v.translation.z = 0.0;
    v.xD0 = 0.0;
    v.xD4 = 0.0;
    v.xD8 = 0.0;
    v.xDC = 0.0;
    v.xE0 = 0.0;
}

/// The floating platform's step, `grGreatBay_801F5E28`: its tilt (`xD8`), slide (`xDC`) and
/// sink (`xE0`) ease toward what fighters pushed this tick (`translation.z`, `xD0`, `xD4`),
/// faster away from rest than back; the model follows and the pushes clear.
#[doc(alias = "grGreatBay_801F5E28")]
pub fn proc_platform(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let Some(j) = r.game.grounds[g.0].u.greatbay3().jobj else {
        return;
    };
    let rate = |r: &Running, away: bool, a: u32, b: u32| reg(p.f(r, if away { a } else { b }));
    let v = r.game.grounds[g.0].u.greatbay3();
    let d = fp::fsubs(reg(v.translation.z), reg(v.xD8));
    let away = fp::fmuls(fp::fneg(reg(v.xD8)), d) < 0.0;
    let k = rate(r, away, p::SLANT_RATE, p::SLANT_REB_RATE);
    let v = r.game.grounds[g.0].u.greatbay3();
    v.xD8 = mem(fp::fmadds(d, k, reg(v.xD8)));
    let d = fp::fsubs(reg(v.xD0), reg(v.xDC));
    let away = fp::fmuls(fp::fneg(reg(v.xDC)), d) < 0.0;
    let k = rate(r, away, p::SLIDE_RATE, p::SLIDE_REB_RATE);
    let v = r.game.grounds[g.0].u.greatbay3();
    v.xDC = mem(fp::fmadds(d, k, reg(v.xDC)));
    let d = fp::fsubs(reg(v.xD4), reg(v.xE0));
    let k = rate(r, d < 0.0, p::DOWN_DOWN_RATE, p::DOWN_UP_RATE);
    let v = r.game.grounds[g.0].u.greatbay3();
    v.xE0 = mem(fp::fmadds(d, k, reg(v.xE0)));
    let (rz, x, y) = (
        v.xD8,
        mem(fp::fadds(reg(v.translation.x), reg(v.xDC))),
        mem(fp::fadds(reg(v.translation.y), reg(v.xE0))),
    );
    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, rz);
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.x = x);
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = y);
    let v = r.game.grounds[g.0].u.greatbay3();
    v.translation.z = 0.0;
    v.xD0 = 0.0;
    v.xD4 = 0.0;
}

/// The floating platform's collision joint's callback, `grGreatBay_801F60C4`: a fighter or
/// another object (collision kinds 1 and 3) on it pushes it by its weight (`x50`, less on
/// landing), tilting and sliding it away from its middle (more the further out, up to 15 units)
/// and sinking it, each within the parameters' limits.
#[doc(alias = "grGreatBay_801F60C4")]
#[allow(clippy::too_many_arguments)]
pub fn platform_touched(
    objs: &mut crate::hsd::Objects,
    data: &melee_data::archive::Archives,
    grounds: &mut crate::hsd::pool::Pool<super::super::structs::Ground>,
    yakumono: Option<DataRef>,
    g: super::super::handles::GroundId,
    coll: &super::super::structs::CollData,
    x50: i32,
    ground_kind: i32,
) {
    let kind = u32::from(coll.x34_flags.b1234);
    if kind != 1 && kind != 3 {
        return;
    }
    let yaku = yakumono.expect("grgreatbay.c: grGb_804D69E0");
    let pf = |off: u32| reg(data.f32(yaku.at(off)));
    let mut x50 = x50;
    if ground_kind == 1 {
        x50 = fp::fctiwz(fp::fmuls(fp::frsp(f64::from(x50)), pf(p::LANDING_RATE)));
    }
    let Some(j) = grounds[g.0].u.greatbay3().jobj else {
        return;
    };
    let pos = crate::lb::joint::world_position(objs, data, Some(j), None);
    let dx = fp::fsubs(reg(coll.cur_pos.x), reg(pos.x));
    let abs = if dx < fp::frsp(0.0) { fp::fneg(dx) } else { dx };
    let mut t = fp::fdivs(abs, 15.0);
    if t > 1.0 {
        t = 1.0;
    }
    let w = fp::frsp(f64::from(x50));
    let rot = fp::fmuls(
        t,
        fp::fmuls(
            reg(0.017_453_292_f32),
            fp::fmadds(w, pf(p::SLANT_MUL), pf(p::SLANT_ADD)),
        ),
    );
    let disp = fp::fmuls(t, fp::fmadds(w, pf(p::SLIDE_MUL), pf(p::SLIDE_ADD)));
    let down = fp::fmadds(w, pf(p::DOWN_MUL), pf(p::DOWN_ADD));
    let v = grounds[g.0].u.greatbay3();
    if dx < 0.0 {
        v.translation.z = mem(fp::fadds(reg(v.translation.z), rot));
        let max = pf(p::SLANT_LIMIT);
        if reg(v.translation.z) > max {
            v.translation.z = mem(max);
        }
        v.xD0 = mem(fp::fadds(reg(v.xD0), disp));
        let max = pf(p::SLIDE_LIMIT);
        if reg(v.xD0) > max {
            v.xD0 = mem(max);
        }
    } else {
        v.translation.z = mem(fp::fsubs(reg(v.translation.z), rot));
        let min = fp::fneg(pf(p::SLANT_LIMIT));
        if reg(v.translation.z) < min {
            v.translation.z = mem(min);
        }
        v.xD0 = mem(fp::fsubs(reg(v.xD0), disp));
        let min = fp::fneg(pf(p::SLIDE_LIMIT));
        if reg(v.xD0) < min {
            v.xD0 = mem(min);
        }
    }
    v.xD4 = mem(fp::fsubs(reg(v.xD4), down));
    let min = fp::fneg(pf(p::DOWN_LIMIT));
    if reg(v.xD4) < min {
        v.xD4 = mem(min);
    }
}

/// A new direction for the turtle other than `current`, `grGreatBay_801F62F8`: drawn by the
/// parameters' weights (front 1, the unused 2, sideways 0, back 3), up to 1,002 tries, else
/// sideways.
#[doc(alias = "grGreatBay_801F62F8")]
pub fn turtle_dir(r: &mut Running, current: i32) -> i32 {
    let p = param(r.game);
    let w = |r: &Running, i: u32| i32::from(p.h(r, p::KAME_DIR_PROB + 2 * i));
    let mut i = 0;
    loop {
        let total = w(r, 0)
            .wrapping_add(w(r, 1))
            .wrapping_add(w(r, 3))
            .wrapping_add(w(r, 2));
        let mut rand = if total != 0 {
            r.game.objs.rng.below(total)
        } else {
            0
        };
        let result = 'pick: {
            for (k, res) in [(2, 1), (1, 2), (0, 0), (3, 3)] {
                rand = rand.wrapping_sub(w(r, k));
                if rand < 0 {
                    break 'pick res;
                }
            }
            panic!("grgreatbay.c:1277: 0");
        };
        if current != result {
            return result;
        }
        let tried = i;
        i += 1;
        if tried > 1000 {
            return 0;
        }
    }
}

/// The turtle's item, `grGreatBay_801F63F4`, once it surfaces: where its shell is (an offset
/// from its part 2), if above the blast zone's bottom, and at the parameters' odds, one of
/// their ten items the rules have on, drawn by weight. Whether it's done with it (false only
/// without the part, or below the blast zone).
#[doc(alias = "grGreatBay_801F63F4")]
pub fn turtle_item(r: &mut Running, gobj: GObjId) -> bool {
    let off = Vec3::new(0.99, 35.72, 0.0);
    let Some(j) = nth_joint(r.game, gobj, 2) else {
        return false;
    };
    let mut pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, Some(j), Some(off));
    pos.z = 0.0;
    if reg(pos.y) <= reg(super::stage::blast_bottom(r.game)) {
        return false;
    }
    let p = param(r.game);
    if reg(r.game.objs.rng.draw_f32()) > reg(p.f(r, p::KAME_ITEM_PROB)) {
        return true;
    }
    let entry = |r: &Running, i: u32| {
        (
            i32::from(p.h(r, p::ITEMS + 4 * i)),
            i32::from(p.h(r, p::ITEMS + 4 * i + 2)),
        )
    };
    let mut total = 0i32;
    for i in 0..10 {
        let (kind, weight) = entry(r, i);
        if kind != -1 && super::super::it::spawn::kind_on(r.game, kind) {
            total = total.wrapping_add(weight);
        }
    }
    if total == 0 {
        return true;
    }
    let mut rand = r.game.objs.rng.below(total);
    let mut selected = -1;
    for i in 0..10 {
        let (kind, weight) = entry(r, i);
        if kind != -1 && super::super::it::spawn::kind_on(r.game, kind) {
            rand = rand.wrapping_sub(weight);
            if rand < 0 {
                selected = kind;
                break;
            }
        }
    }
    if selected == -1 {
        return true;
    }
    let mut spawn = crate::game::structs::SpawnItem {
        kind: selected,
        hold_kind: -1,
        x10: 1,
        pos,
        prev_pos: pos,
        facing_dir: -1.0,
        ..Default::default()
    };
    spawn.x45_flag[0] = 0x80;
    super::super::it::item::spawn_air(r, &mut spawn);
    true
}

/// Each moving vertex at its model joint, `grGreatBay_801F660C` (`grGb_803E4048`).
#[doc(alias = "grGreatBay_801F660C")]
pub fn follow_vertices(r: &mut Running, gobj: GObjId) {
    const VERTS: [(i32, i32); 7] = [
        (32, 20),
        (23, 18),
        (22, 15),
        (29, 16),
        (28, 17),
        (27, 19),
        (39, 21),
    ];
    for (vtx, part) in VERTS {
        let j = nth_joint(r.game, gobj, part);
        assert!(j.is_some(), "grgreatbay.c:1398: jobj");
        let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, None);
        r.game.geometry_mut().set_vertex_pos(vtx, pos.x, pos.y);
    }
}

/// Whether the turtle is up or on its way, `grGreatBay_801F66A4` (a CPU's question).
#[doc(alias = "grGreatBay_801F66A4")]
pub fn turtle_out(game: &mut Game) -> bool {
    if game.stage_info.grkind != super::GR_KIND_GREATBAY {
        return false;
    }
    let Some(t) = map_gobj(game, 1) else {
        return false;
    };
    let g = ground_of(game, t);
    game.grounds[g.0].u.greatbay().xC4 != 0
}

/// The sea's device callback, `grGreatBay_801F6708`: a fighter not hidden off-screen
/// (`x221D_b7`) crossing y = -5 downward splashes there, by its camera box's depth, with a
/// sound. It never acts on the fighter.
#[doc(alias = "grGreatBay_801F6708")]
pub fn sea_device(r: &mut Running, fighter: GObjId) -> bool {
    let fp = r.game.fighter(fighter);
    let (cur, prev) = (fp.cur_pos, fp.prev_pos);
    if !fp.x221D_b7 && reg(cur.y) < fp::fneg(5.0) && reg(prev.y) > fp::fneg(5.0) {
        let b = fp.x890_cameraBox.expect("a fighter without a camera box");
        let depth = r.game.camera.boxes[b].target_ext.v.z;
        let k = fp::fdivs(reg(depth), 10.0);
        let mut at = cur;
        at.y = -5.0;
        splash(r, at, k);
        r.stage_sound(0x57E40);
    }
    false
}

/// A splash at `pos` scaled by `k`, `grGreatBay_801F67A4`.
#[doc(alias = "grGreatBay_801F67A4")]
pub fn splash(r: &mut Running, pos: Vec3, k: f64) {
    let Some(g) = grlib::generator_at(r.game, r.data, 0x7537, 0x1E, pos) else {
        return;
    };
    let s = r.game.objs.generators[g]
        .appsrt
        .expect("grLib_801C96F8 gives its generator an app-SRT");
    let srt = &mut r.game.objs.app_srts[s];
    srt.scale.x = mem(fp::fmuls(reg(srt.scale.x), k));
    srt.scale.y = mem(fp::fmuls(reg(srt.scale.y), k));
    srt.scale.z = mem(fp::fmuls(reg(srt.scale.z), k));
}

/// The stage's shadow check, `grGreatBay_801F6814`: a fighter casts on a joint below its feet.
#[doc(alias = "grGreatBay_801F6814")]
pub fn check_shadow_render(
    game: &mut Game,
    data: &melee_data::archive::Archives,
    pos: Vec3,
    j: Option<JObjId>,
) -> bool {
    let b = crate::lb::joint::world_position(&mut game.objs, data, j, None);
    reg(pos.y) > reg(b.y)
}
