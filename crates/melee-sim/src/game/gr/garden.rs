//! Jungle Japes, `melee/gr/grgarden.c`: the stage (map 2), Cranky Kong's hut (map 1), the
//! Klaptrap that leaps from the river (map 3), the background (map 4), the birds (map 5), the
//! water (map 6) and the sky (map 0).
//!
//! The river below sweeps fighters in it sideways (a wind device the fighter code polls) with a
//! splash as they fall in; the Klaptrap leaps now and then at a random point along the river,
//! biting with its hurtbox item's script (`it::kinds::yaku`'s state 2).

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::{CodeRef, GroundId};
use super::super::state::{Game, Running};
use super::data::{GARDEN_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, nth_joint};
use super::{anime, grlib, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// `grGarden_8020349C`, the river's wind device's callback.
pub const RIVER: u32 = 0x8020_349C;
/// `grGarden_80203498`, the Klaptrap's item's callback for a hit it deals (empty).
pub const KLAPTRAP_BIT: u32 = 0x8020_3498;
/// `grGarden_80202D9C`, the stage's start hook.
const START_HOOK: u32 = 0x8020_2D9C;
/// The river's device type (`ftCo_800C06E8`'s second argument).
const DEVICE_RIVER: u32 = 4;

/// `grGarden_YakumonoParam`, by offset: the river's surface (`x0`) and current (`x4`),
/// Cranky's waits (`x8`, `xC`), the Klaptrap's wait (`x10` plus up to `x14`) and the span it
/// leaps along (`x18` to `x1C`).
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
    Param(game.gr.garden_yakumono.expect("grgarden.c: yakumono_param"))
}

/// A random number below `n`, or 0 for 0.
fn randi_or_zero(r: &mut Running, n: i32) -> i32 {
    if n != 0 { r.game.objs.rng.below(n) } else { 0 }
}

/// A stage object's process or callback, by address: Jungle Japes'.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8020_2D94 | 0x8020_2D98 | 0x8020_2E68 | 0x8020_2F3C | 0x8020_2F40 | 0x8020_2F9C
        | 0x8020_2FA0 | 0x8020_2FFC | 0x8020_3000 | 0x8020_31A0 | 0x8020_3494 => empty(r, gobj),
        0x8020_2D60 => init_sky(r, gobj),
        0x8020_2DC8 => init_stage(r, gobj),
        0x8020_2E48 => proc_stage(r, gobj),
        START_HOOK => start_river(r, gobj),
        0x8020_2E6C => init_back(r, gobj),
        0x8020_2F44 => init_birds(r, gobj),
        0x8020_2FA4 => init_water(r, gobj),
        0x8020_3004 => init_cranky(r, gobj),
        0x8020_3098 => proc_cranky(r, gobj),
        0x8020_31A4 => init_klaptrap(r, gobj),
        0x8020_3250 => proc_klaptrap(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x8020_2B70 => on_init(r),
        0x8020_2B6C => on_demo_init(r),
        0x8020_2C24 => on_load(r),
        0x8020_2C4C => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grGarden_80202D94")]
#[doc(alias = "grGarden_80202D98")]
#[doc(alias = "grGarden_80202E68")]
#[doc(alias = "grGarden_80202F3C")]
#[doc(alias = "grGarden_80202F40")]
#[doc(alias = "grGarden_80202F9C")]
#[doc(alias = "grGarden_80202FA0")]
#[doc(alias = "grGarden_80202FFC")]
#[doc(alias = "grGarden_80203000")]
#[doc(alias = "grGarden_802031A0")]
#[doc(alias = "grGarden_80203494")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `grGarden_80202C78`.
#[doc(alias = "grGarden_80202C78")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grGd_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &GARDEN_CALLBACKS[map_id as usize]
}

/// Jungle Japes' setup, `grGarden_80202B70`: its parameters, maps 0, 4, 5, 6, 1, 3 and 2,
/// camera limits and blast zones, and the river's sound unless the stage is the single-player
/// one's or the match is a mode without it.
#[doc(alias = "grGarden_80202B70")]
pub fn on_init(r: &mut Running) {
    r.game.gr.garden_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [0, 4, 5, 6, 1, 3, 2] {
        create(r, map_id, setup::SETUP_FRAME);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    if super::stage::stkind(r.game) != 62 && r.game.vs_x6() == 0 {
        r.stage_sound(0x6_4194);
    }
}

/// `grGarden_80202B6C` does nothing (only a demo runs it).
#[doc(alias = "grGarden_80202B6C")]
pub fn on_demo_init(_r: &mut Running) {}

/// Jungle Japes' load hook, `grGarden_80202C24`: a stage sound.
#[doc(alias = "grGarden_80202C24")]
pub fn on_load(r: &mut Running) {
    r.stage_sound(0x6_4193);
}

/// Jungle Japes' start, `grGarden_80202C4C`: the enemy spawner, without enemies.
#[doc(alias = "grGarden_80202C4C")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// A map's animation and no overlay callbacks (`x8_callback`, `xC_callback`), drawn in `pass`.
fn init_plain(r: &mut Running, gobj: GObjId, pass: u8) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.xC_callback = None;
    gp.x11_flags.b012 = pass;
}

/// The sky's setup (map 0), `stageGObj0_OnInit`: its animation.
#[doc(alias = "stageGObj0_OnInit@80202D60")]
pub fn init_sky(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The stage's start hook, `grGarden_80202D9C`: the river registered as the fighters' wind
/// device.
#[doc(alias = "grGarden_80202D9C")]
pub fn start_river(r: &mut Running, gobj: GObjId) {
    super::oldpupupu::register_wind(r.game, gobj, DEVICE_RIVER, CodeRef(RIVER));
}

/// The stage's setup (map 2), `grGarden_80202DC8`: its animation, no overlay callbacks, the
/// river registered when the match starts, and it receives shadows.
#[doc(alias = "grGarden_80202DC8")]
pub fn init_stage(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.xC_callback = None;
    setup::add_start_hook(r.game, gobj, CodeRef(START_HOOK));
    let gp = &mut r.game.grounds[g.0];
    gp.x11_flags.b012 = 1;
    gp.x10_flags.b5 = true;
}

/// The stage's process (map 2), `grGarden_80202E48`: the wind sources age.
#[doc(alias = "grGarden_80202E48")]
pub fn proc_stage(r: &mut Running, _gobj: GObjId) {
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
}

/// The background's setup (map 4), `grGarden_80202E6C`: its animation, and six of its joints
/// as the stage's guide points.
#[doc(alias = "grGarden_80202E6C")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    init_plain(r, gobj, 1);
    let j = [4, 3, 7, 6, 10, 9].map(|d| nth_joint(r.game, gobj, d));
    super::ground::set_guide_points(r.game, r.data, j);
}

/// The birds' setup (map 5), `grGarden_80202F44`: their animation.
#[doc(alias = "grGarden_80202F44")]
pub fn init_birds(r: &mut Running, gobj: GObjId) {
    init_plain(r, gobj, 1);
}

/// The water's setup (map 6), `grGarden_80202FA4`: its animation, drawn behind.
#[doc(alias = "grGarden_80202FA4")]
pub fn init_water(r: &mut Running, gobj: GObjId) {
    init_plain(r, gobj, 2);
}

/// Cranky Kong's setup (map 1), `grGarden_80203004`: his first animation, held for a random
/// count of its runs.
#[doc(alias = "grGarden_80203004")]
pub fn init_cranky(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    r.game.grounds[g.0].u.garden().xc4 = 0;
    let n = param(r.game).i(r.data, 8);
    let wait = randi_or_zero(r, n) + 1;
    r.game.grounds[g.0].u.garden().xc8 = wait;
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.xC_callback = None;
    gp.x11_flags.b012 = 1;
}

/// Cranky Kong's process (map 1), `grGarden_80203098`: each time his animation ends or loops,
/// it plays again until its count runs out; then the next of his four animations starts, the
/// first and the third held for random counts.
#[doc(alias = "grGarden_80203098")]
pub fn proc_cranky(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    if !(anime::finished(r.game, gobj, 0, 7) || anime::rewound(r.game, gobj, 0, 7)) {
        return;
    }
    let v = r.game.grounds[g.0].u.garden();
    if v.xc8 != 0 {
        v.xc8 -= 1;
        return;
    }
    v.xc4 += 1;
    if 3 < v.xc4 {
        v.xc4 = 0;
    }
    let anim = v.xc4;
    let p = param(r.game);
    if anim == 0 {
        let n = p.i(r.data, 8);
        let w = randi_or_zero(r, n) + 1;
        r.game.grounds[g.0].u.garden().xc8 = w;
    } else if anim == 2 {
        let n = p.i(r.data, 0xC);
        let w = randi_or_zero(r, n) + 1;
        r.game.grounds[g.0].u.garden().xc8 = w;
    }
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, anim);
}

/// The Klaptrap's setup (map 3), `grGarden_802031A4`: its hurtbox item on part 8, and a
/// random wait.
#[doc(alias = "grGarden_802031A4")]
pub fn init_klaptrap(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let part = nth_joint(r.game, gobj, 8);
    let item =
        super::material::stage_item(r, 0, 0, g, part, Some(CodeRef(KLAPTRAP_BIT)), None, None);
    r.game.grounds[g.0].u.garden2().xc4 = item;
    let p = param(r.game);
    let n = p.i(r.data, 0x14);
    let wait = p.i(r.data, 0x10).wrapping_add(randi_or_zero(r, n));
    let v = r.game.grounds[g.0].u.garden2();
    v.xcc = wait;
    v.xc8 = 0;
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.xC_callback = None;
    gp.x11_flags.b012 = 1;
}

/// The Klaptrap's process (map 3), `grGarden_80203250`. Waiting (0), it counts down; then its
/// item takes state 2 (its bite's script), its animation starts at a random whole distance
/// along the river's span, and it leaps (1), with splashes at four ticks of the leap; when the
/// animation ends its item goes back to state 0, its animations are dropped and a new wait
/// starts (one tick longer, the leap's count going on).
#[doc(alias = "grGarden_80203250")]
pub fn proc_klaptrap(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = nth_joint(r.game, gobj, 0);
    let p = param(r.game);
    let v = r.game.grounds[g.0].u.garden2();
    match v.xc8 {
        0 => {
            if v.xcc != 0 {
                v.xcc -= 1;
                return;
            }
            let item = v.xc4.expect("grgarden.c: the Klaptrap's item");
            super::super::it::item::change_state(r, item, 2, 2);
            let m = map_id(r.game, gobj);
            anime::start_map(r.game, r.data, gobj, m, 0);
            let (lo, hi) = (reg(p.f(r.data, 0x18)), reg(p.f(r.data, 0x1C)));
            let span = fp::fsubs(hi, lo);
            let span = fp::fctiwz(if span < 0.0 { fp::fneg(span) } else { span });
            let k = randi_or_zero(r, span);
            let x = mem(fp::fadds(lo, fp::frsp(f64::from(k))));
            let j = j.expect("grgarden.c: the Klaptrap's part");
            jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.x = x);
            let v = r.game.grounds[g.0].u.garden2();
            v.xcc = 0;
            v.xc8 = 1;
        }
        1 => {
            let t = v.xcc;
            if t == 38 || t == 148 {
                r.stage_sound(410_001);
            } else if t == 110 || t == 210 {
                r.stage_sound(410_002);
            }
            if anime::finished(r.game, gobj, 0, 7) {
                let item = r.game.grounds[g.0].u.garden2().xc4;
                let item = item.expect("grgarden.c: the Klaptrap's item");
                super::super::it::item::change_state(r, item, 0, 2);
                jobj::remove_anim_all(&mut r.game.objs, j);
                let n = p.i(r.data, 0x14);
                let wait = p.i(r.data, 0x10).wrapping_add(randi_or_zero(r, n));
                let v = r.game.grounds[g.0].u.garden2();
                v.xcc = wait;
                v.xc8 = 0;
            }
            let v = r.game.grounds[g.0].u.garden2();
            v.xcc = v.xcc.wrapping_add(1);
        }
        _ => {}
    }
}

/// The Klaptrap's item's callback for a hit it deals, `grGarden_80203498`: nothing.
#[doc(alias = "grGarden_80203498")]
pub fn klaptrap_bit(_r: &mut Running, _item: GObjId, _g: GroundId) {}

/// The river's pull on a fighter, `grGarden_8020349C`: a fighter below the surface is swept
/// along by the current (the parameters' `x4` in x), and one that has just fallen in splashes,
/// as big as its camera box's depth, with the splash's sound.
#[doc(alias = "grGarden_8020349C")]
pub fn river(r: &mut Running, _ground: GObjId, fighter: GObjId) -> Option<Vec3> {
    let p = param(r.game);
    let surface = reg(p.f(r.data, 0));
    let mut cur = r.game.ft_pos(fighter);
    let prev = r.game.fighter(fighter).prev_pos;
    if !(reg(cur.y) < surface) {
        return None;
    }
    if reg(prev.y) > surface {
        let size = fp::fdivs(reg(r.game.ft_camera_depth(fighter)), 10.0);
        cur.y = -30.0;
        splash(r, mem(size), cur);
        r.stage_sound(410_000);
    }
    Some(Vec3::new(p.f(r.data, 4), 0.0, 0.0))
}

/// A splash at `pos`, `grGarden_80203560`: the stage's effect 0x753A, scaled by `size`.
#[doc(alias = "grGarden_80203560")]
pub fn splash(r: &mut Running, size: f32, pos: Vec3) {
    let Some(g) = grlib::generator_at(r.game, r.data, 0x753A, 0x1E, pos) else {
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

/// The stage's shadow check, `grGarden_802035D0`: a fighter casts on a joint below its feet.
#[doc(alias = "grGarden_802035D0")]
pub fn check_shadow_render(
    game: &mut Game,
    data: &Archives,
    pos: Vec3,
    j: Option<crate::hsd::jobj::JObjId>,
) -> bool {
    let b = world_position(&mut game.objs, data, j, None);
    pos.y > b.y
}

/// The lowest the camera's eye goes on Jungle Japes, `grGarden_80203624`.
#[doc(alias = "grGarden_80203624")]
pub fn camera_floor() -> f32 {
    -20.0
}
