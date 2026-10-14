//! Fountain of Dreams, `melee/gr/grizumi.c`: the fountain (map 3) with its two side platforms
//! (map 4 objects), which rise and sink on the water's columns and now and then drop under
//! the surface for a while; the water's reflection, a camera (`grIzumi_801CCD98`) that renders
//! the stage mirrored into a texture the fountain's water shows; the platforms' reflections in
//! that water (map 2); the sky's stars (`GrdIzumiStar_TopN_joint`) and the background (maps 0,
//! 1 and 5 to 10).

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::handles::CodeRef;
use super::super::state::{Game, Running};
use super::data::{IZUMI_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, nth_joint, scale};
use super::{anime, grlib, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

/// The fountain's map id.
pub const MAP_FOUNTAIN: i32 = 3;
/// The platforms' reflections' map id.
pub const MAP_REFLECTIONS: i32 = 2;
/// The platforms' map id.
pub const MAP_PLATFORM: i32 = 4;

/// The fields of `grIzumi_YakumonoParam` the code reads, by offset.
#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn f(self, r: &Running, off: u32) -> f32 {
        r.data.f32(self.0.at(off))
    }
}

fn param(game: &Game) -> Param {
    Param(game.gr.izumi_yakumono.expect("grizumi.c: yakumono_param"))
}

/// A stage object's process or callback, by address: Fountain of Dreams'.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x801C_BE08 | 0x801C_BE0C | 0x801C_BE5C | 0x801C_BE60 | 0x801C_C338 | 0x801C_CA10
        | 0x801C_CA5C | 0x801C_CA60 | 0x801C_CB10 | 0x801C_CB14 => empty(r, gobj),
        0x801C_BDD4 => init_sky(r, gobj),
        0x801C_BE10 => init_back(r, gobj),
        0x801C_BE64 => init_fountain(r, gobj),
        0x801C_C0D4 => proc_fountain(r, gobj),
        0x801C_C33C => init_platform(r, gobj),
        0x801C_C358 => proc_platform(r, gobj),
        0x801C_CA14 => init_piece(r, gobj),
        0x801C_CA64 => init_reflections(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x801C_BB88 => on_init(r),
        0x801C_BC0C => on_load(r),
        0x801C_BCBC => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grIzumi_801CBE08")]
#[doc(alias = "grIzumi_801CBE5C")]
#[doc(alias = "grIzumi_801CCB10")]
// Also ports the background pieces' process `grIzumi_801CCA5C`; aliased once a check reaches
// it (no VS match makes maps 5 to 10).
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Where the reflections', the platforms' and the stars' calls put their scale joints'
/// descriptors on the stack (`setup::SETUP_FRAME`).
const REFLECTIONS_FRAME: u32 = 0x804E_E970;
const PLATFORM_FRAME: u32 = 0x804E_E918;
const STARS_FRAME: u32 = 0x804E_E988;

/// Creates one of the stage's objects and sets it up, `grIzumi_801CBCE8`.
#[doc(alias = "grIzumi_801CBCE8")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grIz_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &IZUMI_CALLBACKS[map_id as usize]
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// Fountain of Dreams' setup, `grIzumi_801CBB88`: its parameters, maps 0, 1 and 3, camera
/// limits and blast zones.
#[doc(alias = "grIzumi_801CBB88")]
pub fn on_init(r: &mut Running) {
    r.game.gr.izumi_yakumono = r.game.stage_info.yakumono_param;
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    create(r, 0, setup::SETUP_FRAME);
    create(r, 1, setup::SETUP_FRAME);
    create(r, MAP_FOUNTAIN, setup::SETUP_FRAME);
    check_archive(r.game, r.data, 3);
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
}

/// `grAnime_801C8780`: looks up the stage's light GObj (`Ground_801C498C`) and asserts the
/// map's archive is loaded; nothing else.
#[doc(alias = "grAnime_801C8780")]
pub fn check_archive(game: &Game, data: &melee_data::archive::Archives, map_id: i32) {
    assert!(
        super::datfiles::archive_for(game, data, map_id).is_some(),
        "granime.c: archive"
    );
}

/// `grIzumi_801CBB84` does nothing. Ports `grIzumi_801CBB84`; aliased once a check reaches it
/// (only a demo runs it).
pub fn on_demo_init(_r: &mut Running) {}

/// The stage's load hook, `grIzumi_OnLoad`: every animation of the stage's lights loops.
#[doc(alias = "grIzumi_OnLoad")]
pub fn on_load(r: &mut Running) {
    let mut g = r.game.world.first_in(3);
    while let Some(gobj) = g {
        let o = r.game.world.gobj(gobj);
        if o.classifier == 0xC {
            if let Some(super::super::state::Obj::LObj(first)) = o.obj {
                for a in setup::light_anims(r.game, first, u32::MAX) {
                    crate::hsd::aobj::set_flags(
                        &mut r.game.objs.aobjs,
                        Some(a),
                        crate::hsd::aobj::LOOP,
                    );
                }
            }
            return;
        }
        g = o.next;
    }
}

/// Fountain of Dreams' start, `grIzumi_OnStart`: the enemy spawner, without enemies.
#[doc(alias = "grIzumi_OnStart")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

/// Map 0's setup, `grIzumi_801CBDD4`: its animation.
#[doc(alias = "grIzumi_801CBDD4")]
pub fn init_sky(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// Map 1's setup, `grIzumi_801CBE10`: its animation in layer 1.
#[doc(alias = "grIzumi_801CBE10")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x11_flags.b012 = 1;
}

/// The fountain's setup (map 3), `grIzumi_801CBE64`: its collision and animation, the
/// platforms' joints, the reflection camera and the water's texture that shows it, its render
/// callback, the stars, two particle generators, the platforms' reflections (map 2), and the two
/// platforms where joints 4 and 6 stand.
#[doc(alias = "grIzumi_801CBE64")]
pub fn init_fountain(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x11_flags.b012 = 1;
    r.game.grounds[g.0].x10_flags.b5 = true;
    let (left, right) = (nth_joint(r.game, gobj, 1), nth_joint(r.game, gobj, 2));
    let v = r.game.grounds[g.0].u.izumi();
    v.xD0 = left;
    v.xD4 = right;
    let camera = create_reflection(r);
    r.game.grounds[g.0].u.izumi().xC8 = Some(camera);
    let image = r.game.gr.izumi.reflection.as_ref().and_then(|v| v.image);
    let tobj = find_tobj(r.game, gobj, image);
    match tobj {
        Some(t) => {
            let o = &mut r.game.objs.tobjs[t];
            o.src = 0;
            o.wrap_s = 0;
            o.wrap_t = 0;
            o.flags = (o.flags & !0x1FF) | 0x103;
        }
        None => panic!("grizumi.c:387:Warning: not found tobj !"),
    }
    r.game.grounds[g.0].u.izumi().xC4 = tobj;
    r.game.world.gobj_mut(gobj).render = Some(CodeRef(RENDER_FOUNTAIN));
    create_stars(r);
    grlib::generator_at(r.game, r.data, 0x7534, 0x1E, Vec3::new(0.0, 0.0, 0.0));
    let s = reg(scale(r.game));
    let m = |v: i32| mem(fp::fmuls(fp::frsp(f64::from(v)), s));
    grlib::generator_at(r.game, r.data, 0x7536, 0x1E, Vec3::new(m(0), m(1), m(-27)));
    let refl = create(r, MAP_REFLECTIONS, REFLECTIONS_FRAME);
    r.game.grounds[g.0].u.izumi().xCC = refl;
    let rg = ground_of(r.game, refl.expect("grizumi.c: the reflections"));
    r.game.grounds[rg.0].x18 = Some(camera);
    let p = param(r.game);
    let (h0, h1, home) = (p.f(r, 0), p.f(r, 8), p.f(r, 0xC));
    let (left, right) = {
        let v = r.game.grounds[g.0].u.izumi();
        (v.xD0, v.xD4)
    };
    for (part, height, line, column) in [(4, h0, 0, left), (6, h1, 1, right)] {
        let j = nth_joint(r.game, gobj, part);
        let at = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, None);
        let plat = create_platform(r, height, at, line, column);
        let pg = ground_of(r.game, plat);
        r.game.grounds[pg.0].u.izumi3().xDC = home;
        r.game.grounds[pg.0].x10_flags.b3 = true;
        r.game.grounds[pg.0].x18 = Some(camera);
    }
}

/// The fountain's render callback, `grIzumi_801CD220`.
pub const RENDER_FOUNTAIN: u32 = 0x801C_D220;
/// The reflection camera's render callback, `grIzumi_801CCEA0`.
pub const RENDER_REFLECTION: u32 = 0x801C_CEA0;
/// The stars' render callback, `grIzumi_801CCB90`.
pub const RENDER_STARS: u32 = 0x801C_CB90;

/// The stage's render callbacks, by address.
pub fn run_render(r: &mut Running, gobj: GObjId, cb: u32, pass: i32) -> bool {
    match cb {
        RENDER_FOUNTAIN => render_fountain(r, gobj, pass),
        RENDER_REFLECTION => render_reflection(r, gobj),
        RENDER_STARS => render_stars(r, gobj, pass),
        _ => return false,
    }
    true
}

/// The first texture of the fountain's model showing `image`, `grIzumi_801CD090`: depth first,
/// past joints holding splines or particles and instances' children.
#[doc(alias = "grIzumi_801CD090")]
pub fn find_tobj(
    game: &Game,
    gobj: GObjId,
    image: Option<DataRef>,
) -> Option<crate::hsd::tobj::TObjId> {
    let image = image.map(crate::hsd::tobj::ImageRef::Data);
    super::stadium::find_tobj(game, gobj, image).map(|(t, _)| t)
}

/// The stars (`GrdIzumiStar_TopN_joint`), `grIzumi_801CCB18`: a stage object of their own in
/// camera layer 2, drawn with a larger point size.
#[doc(alias = "grIzumi_801CCB18")]
pub fn create_stars(r: &mut Running) {
    let archive = super::datfiles::main_archive(r.game).archive;
    let joint = r
        .data
        .get(archive)
        .public("GrdIzumiStar_TopN_joint")
        .map(|offset| DataRef { archive, offset });
    let Some(joint) = joint else { return };
    let Some(sub) = setup::stage_gobj_from_joint(r, joint, -1, STARS_FRAME) else {
        return;
    };
    let g = ground_of(r.game, sub);
    r.game.world.setup_gx_link(sub, CodeRef(RENDER_STARS), 3, 0);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// The stars' render callback, `grIzumi_801CCB90`: a point size of 18, then drawn as any stage
/// object is.
#[doc(alias = "grIzumi_801CCB90")]
pub fn render_stars(r: &mut Running, gobj: GObjId, pass: i32) {
    r.game
        .objs
        .display
        .draws
        .push(crate::hsd::display::Draw::PointSize(18, 0));
    r.render_stage_object(gobj, pass);
}

/// The fountain's render callback, `grIzumi_801CD220`: the water's texture takes the
/// reflection's texture matrix, then drawn as any stage object is.
#[doc(alias = "grIzumi_801CD220")]
pub fn render_fountain(r: &mut Running, gobj: GObjId, pass: i32) {
    let g = ground_of(r.game, gobj);
    let tobj = r.game.grounds[g.0].u.izumi().xC4.expect("grizumi.c: tobj");
    let m = r
        .game
        .gr
        .izumi
        .reflection
        .as_ref()
        .and_then(|v| v.texture_matrix)
        .expect("the reflection's texture matrix before its camera rendered");
    r.game.objs.tobjs[tobj].mtx = m.copied();
    r.render_stage_object(gobj, pass);
}

/// The fountain's process, `grIzumi_801CC0D4`: the platforms' reflections follow the platforms'
/// joints, hidden while below the surface; and the wind sources.
#[doc(alias = "grIzumi_801CC0D4")]
pub fn proc_fountain(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.izumi();
    if let Some(refl) = v.xCC {
        let (left, right) = (v.xD0, v.xD4);
        let rg = ground_of(r.game, refl);
        let rv = r.game.grounds[rg.0].u.izumi2();
        let pairs = [(left, rv.xC4), (right, rv.xC8)];
        for (src, dst) in pairs {
            let Some(src) = src else { continue };
            let pos = r.game.objs.jobjs[src].translate;
            let dst = dst.expect("grizumi.c: a reflection's joint");
            jobj::set_srt(&mut r.game.objs.jobjs, dst, |o| o.translate = pos);
            let hidden = r.game.objs.jobjs[dst].flags & jobj::HIDDEN != 0;
            if reg(pos.y) < 0.0 {
                if !hidden {
                    jobj::set_flags_all(&mut r.game.objs.jobjs, Some(dst), jobj::HIDDEN);
                }
            } else if hidden {
                jobj::clear_flags_all(&mut r.game.objs.jobjs, Some(dst), jobj::HIDDEN);
            }
        }
    }
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
}

/// A platform's setup (map 4), `grIzumi_801CC33C`: no callbacks.
#[doc(alias = "grIzumi_801CC33C")]
pub fn init_platform(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.xC_callback = None;
}

/// `rand_range`: between the two, whichever is lower, drawn only if they differ.
fn rand_range(r: &mut Running, a: i32, b: i32) -> i32 {
    let randi = |r: &mut Running, n: i32| if n != 0 { r.game.objs.rng.below(n) } else { 0 };
    if a > b {
        b.wrapping_add(randi(r, a.wrapping_sub(b)))
    } else if a < b {
        a.wrapping_add(randi(r, b.wrapping_sub(a)))
    } else {
        a
    }
}

/// A random wait between two of the parameters' floats, as integers.
fn wait_between(r: &mut Running, a: u32, b: u32) -> i16 {
    let p = param(r.game);
    let (a, b) = (fp::fctiwz(reg(p.f(r, a))), fp::fctiwz(reg(p.f(r, b))));
    rand_range(r, a, b) as i16
}

/// A platform's process, `grIzumi_801CC358`: still for a while (state 1), then a new height
/// (the water's column rising or falling by a random step, or once in a while under the
/// surface), moving toward it (state 2) and, once under, hidden for a while (states 3 and 4);
/// the column's joint scales with the height and the collision line follows the platform.
#[doc(alias = "grIzumi_801CC358")]
pub fn proc_platform(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let p = param(r.game);
    let mut moved = false;
    match r.game.grounds[g.0].u.izumi3().xC4 {
        0 => {
            r.game.grounds[g.0].u.izumi3().xC4 = 1;
            let w = wait_between(r, 0x3C, 0x38);
            r.game.grounds[g.0].u.izumi3().xC6 = w;
            moved = true;
        }
        1 => {
            let v = r.game.grounds[g.0].u.izumi3();
            let t = v.xC6;
            v.xC6 = t.wrapping_sub(1);
            if t < 0 {
                let rand = reg(r.game.objs.rng.draw_f32());
                let (sink, step, keep) = (p.f(r, 0x40), p.f(r, 0x48), p.f(r, 0x44));
                let total = fp::fadds(reg(step), fp::fadds(reg(sink), reg(keep)));
                let mut f = fp::fmuls(total, rand);
                f = fp::fsubs(f, reg(sink));
                if f < 0.0 {
                    let v = r.game.grounds[g.0].u.izumi3();
                    v.xC4 = 2;
                    v.xD4 = -1.0;
                } else {
                    f = fp::fsubs(f, reg(step));
                    if f < 0.0 {
                        let fff = reg(r.game.objs.rng.draw_f32());
                        let (lo, hi) = (p.f(r, 0x18), p.f(r, 0x1C));
                        let ff = fp::fmadds(fp::fsubs(reg(hi), reg(lo)), fff, reg(lo));
                        r.game.grounds[g.0].u.izumi3().xC4 = 2;
                        let v = r.game.grounds[g.0].u.izumi3();
                        let (h, home) = (reg(v.xD0), reg(v.xDC));
                        let up = if h < home {
                            !(reg(r.game.objs.rng.draw_f32()) < reg(p.f(r, 0x30)))
                        } else if h > home {
                            reg(r.game.objs.rng.draw_f32()) < reg(p.f(r, 0x34))
                        } else {
                            reg(r.game.objs.rng.draw_f32()) < 0.5
                        };
                        let v = r.game.grounds[g.0].u.izumi3();
                        v.xD4 = mem(if up {
                            fp::fadds(reg(v.xD4), ff)
                        } else {
                            fp::fsubs(reg(v.xD4), ff)
                        });
                        let (top, bottom) = (p.f(r, 0x20), p.f(r, 0x24));
                        let v = r.game.grounds[g.0].u.izumi3();
                        if reg(v.xD4) > reg(top) {
                            v.xD4 = top;
                        } else if reg(v.xD4) < reg(bottom) {
                            v.xD4 = bottom;
                        }
                    } else {
                        let w = wait_between(r, 0x3C, 0x38);
                        r.game.grounds[g.0].u.izumi3().xC6 = w;
                    }
                }
            }
        }
        2 => {
            let (rise, fall, bottom) = (p.f(r, 0x28), p.f(r, 0x2C), p.f(r, 0x24));
            let v = r.game.grounds[g.0].u.izumi3();
            let f = fp::fsubs(reg(v.xD4), reg(v.xD0));
            if f > fp::frsp(0.0) {
                if f < reg(rise) {
                    v.xD0 = v.xD4;
                    v.xC4 = 0;
                } else {
                    v.xD0 = mem(fp::fadds(reg(v.xD0), reg(rise)));
                }
            } else if f < fp::frsp(0.0) {
                if fp::fneg(f) < reg(fall) {
                    v.xD0 = v.xD4;
                    v.xC4 = if reg(v.xD0) < reg(bottom) { 3 } else { 0 };
                } else {
                    v.xD0 = mem(fp::fsubs(reg(v.xD0), reg(fall)));
                }
            } else {
                v.xC4 = 0;
            }
            moved = true;
        }
        3 => {
            r.game.grounds[g.0].u.izumi3().xC4 = 4;
            let w = wait_between(r, 0x50, 0x4C);
            r.game.grounds[g.0].u.izumi3().xC6 = w;
            jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
            jobj::remove_anim_all(&mut r.game.objs, j);
            let y = r.game.objs.jobjs[j.expect("grizumi.c: jobj")].translate.y;
            let y = mem(fp::frsp(fp::fadd(reg(y), fp::fneg(1.0))));
            let column = r.game.grounds[g.0].u.izumi3().xCC;
            let c = column.expect("grizumi.c: a platform's column");
            jobj::set_srt(&mut r.game.objs.jobjs, c, |o| o.translate.y = y);
        }
        4 => {
            let v = r.game.grounds[g.0].u.izumi3();
            let t = v.xC6;
            v.xC6 = t.wrapping_sub(1);
            if t < 0 {
                v.xC4 = 2;
                v.xD4 = v.xDC;
                jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
                anime::start(r.game, r.data, gobj, 0, 7, 0, 0.0, 1.0, true);
                moved = true;
            }
        }
        _ => {}
    }
    if moved && let Some(child) = j.and_then(|j| r.game.objs.jobjs[j].child) {
        let v = r.game.grounds[g.0].u.izumi3();
        let mut f = fp::frsp(fp::fdiv(reg(v.xD0), reg(v.xD8)));
        if f < f64::from(0.01_f32) {
            f = f64::from(0.01_f32);
        }
        let (h, column) = (v.xD0, v.xCC);
        jobj::set_srt(&mut r.game.objs.jobjs, child, |o| {
            o.scale.x = mem(fp::fmadds(0.5, f, 0.5))
        });
        jobj::set_srt(&mut r.game.objs.jobjs, child, |o| o.scale.y = mem(f));
        let c = column.expect("grizumi.c: a platform's column");
        jobj::set_srt(&mut r.game.objs.jobjs, c, |o| o.translate.y = h);
    }
    let line_joint = i32::from(r.game.grounds[g.0].u.izumi3().xC8);
    super::ground::update_coll_joint(r.game, r.data, line_joint);
}

/// The background pieces' setup (maps 5 to 10), `grIzumi_801CCA14`: their animation, no
/// callbacks. Ports `grIzumi_801CCA14`; aliased once a check reaches it (no VS match makes
/// these maps).
pub fn init_piece(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.xC_callback = None;
}

/// The platforms' reflections' setup (map 2), `grIzumi_801CCA64`: their animation, drawn in
/// camera layer 2 rather than 3, and their two joints, hidden.
#[doc(alias = "grIzumi_801CCA64")]
pub fn init_reflections(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.xC_callback = None;
    r.game.world.gx_unlink(gobj);
    r.game.world.setup_gx_link(gobj, CodeRef(0x801C_5DB0), 2, 0);
    let a = nth_joint(r.game, gobj, 2);
    jobj::set_flags_all(&mut r.game.objs.jobjs, a, jobj::HIDDEN);
    let b = nth_joint(r.game, gobj, 3);
    jobj::set_flags_all(&mut r.game.objs.jobjs, b, jobj::HIDDEN);
    let v = r.game.grounds[g.0].u.izumi2();
    v.xC4 = a;
    v.xC8 = b;
}

/// Makes a platform at `pos` (map 4), `grIzumi_801CCBDC`: its height, its collision line
/// joint `line`, its column's joint, under the surface (state 3) if its height is below 0,
/// else its animation; the column's full height from its joint 2 (else 45), and its first
/// tick.
#[doc(alias = "grIzumi_801CCBDC")]
pub fn create_platform(
    r: &mut Running,
    height: f32,
    pos: Vec3,
    line: i32,
    column: Option<JObjId>,
) -> GObjId {
    let gobj = create(r, MAP_PLATFORM, PLATFORM_FRAME).expect("grizumi.c: oioi...");
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("grizumi.c: oioi...");
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate = pos);
    let v = r.game.grounds[g.0].u.izumi3();
    v.xD4 = height;
    v.xD0 = height;
    v.xC8 = line as i16;
    v.xCC = column;
    if reg(height) < 0.0 {
        v.xC4 = 3;
    } else {
        v.xC4 = 0;
        anime::start(r.game, r.data, gobj, 0, 7, 0, 0.0, 1.0, true);
    }
    let full = match nth_joint(r.game, gobj, 2) {
        Some(top) => {
            let w = crate::lb::joint::world_position(&mut r.game.objs, r.data, Some(top), None);
            mem(fp::fdivs(
                fp::fsubs(reg(w.y), reg(pos.y)),
                reg(scale(r.game)),
            ))
        }
        None => 45.0,
    };
    r.game.grounds[g.0].u.izumi3().xD8 = full;
    proc_platform(r, gobj);
    gobj
}

/// The reflection camera's user data (`IzumiReflection`, from the heap): the texture matrix its
/// render leaves for the water (none until it first renders: the block comes uncleared), and
/// the stage archive's image descriptor it renders into, which it rewrites.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
pub struct Reflection {
    pub texture_matrix: Option<crate::mtx::Mtx34>,
    /// The archive's `GrdIzumi_cd_wt_GrdIzumiDummy1_1_image_desc`.
    pub image: Option<DataRef>,
    /// What the stage wrote over that descriptor: an 80x60 image of format 4 in the preloaded
    /// buffer 2001, or with nothing preloaded, a buffer of the scene's heap (`lb_800121FC`).
    pub desc: crate::game::structs::HSD_ImageDesc,
    /// Where the original's heap put this user data (`docs/engine.md`).
    pub addr: u32,
}

/// The stage's state besides its objects'.
#[derive(melee_derive::CloneFrom, Debug, Default)]
pub struct Globals {
    /// The reflection camera's user data, once made.
    pub reflection: Option<Reflection>,
}

/// `ReflectCObjDesc`, the reflection camera's descriptor.
const REFLECT_CAMERA: crate::hsd::cobj::StaticDesc = crate::hsd::cobj::StaticDesc {
    flags: 0,
    projection_type: 1,
    viewport: [0, 80, 0, 60],
    scissor: [0, 80, 0, 60],
    eye: Vec3::new(0.0, 0.0, 1.0),
    interest: Vec3::new(0.0, 0.0, 0.0),
    roll: 0.0,
    near: 0.1,
    far: 32768.0,
    projection: [30.0, 1.217_333_3, 0.0, 0.0],
};

/// The reflection camera, `grIzumi_801CCD98`: a GObj (class 0x11, link 0x12) with a camera
/// rendering an 80x60 texture (render priority 2), its user data the [`Reflection`], whose
/// image is the stage archive's descriptor the water's texture shows, rewritten for the render.
#[doc(alias = "grIzumi_801CCD98")]
pub fn create_reflection(r: &mut Running) -> GObjId {
    use super::super::state::{OBJ_CAMERA, Obj, UserData};
    let gobj = r.game.world.create(0x11, 0x12, 0);
    let c = crate::lb::cobj::load_cobj(
        &mut r.game.objs,
        r.data,
        crate::lb::cobj::CObjDesc::Static(&REFLECT_CAMERA),
    );
    r.game.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
    r.game
        .world
        .setup_gx_link_max(gobj, CodeRef(RENDER_REFLECTION), 2, true);
    let addr = crate::hsd::heap::mem_alloc(0x34);
    r.game
        .world
        .init_user_data(gobj, 3, UserData::IzumiReflection);
    let head = super::datfiles::archive_for(r.game, r.data, 3).expect("grDatFiles_801C6330(3)");
    let archive = head.0.archive;
    let image = r
        .data
        .get(archive)
        .public("GrdIzumi_cd_wt_GrdIzumiDummy1_1_image_desc")
        .map(|offset| DataRef { archive, offset });
    assert!(
        image.is_some(),
        "not found mirror image desc! (GrdIzumi_cd_wt_GrdIzumiDummy1_1_image_desc)"
    );
    // `lb_800121FC`'s buffer: preload entry 2001 where the preload cache holds it (a match the
    // VS mode starts from its menus), else the scene's heap's (the match a replay's playback
    // boots into).
    if !r.game.ft.data.files.buffer_preloaded(2001) {
        crate::hsd::heap::mem_alloc(
            crate::hsd::heap::tex_buffer_size(80, 60, 4).next_multiple_of(32),
        );
    }
    r.game.gr.izumi.reflection = Some(Reflection {
        texture_matrix: None,
        image,
        desc: super::stadium::screen_image(80, 60, 4),
        addr,
    });
    gobj
}

/// The reflection camera's render callback, `grIzumi_801CCEA0`: the fighters drawn with the
/// magnifier's callback (`ftDrawCommon_80081140`), the camera taking the game camera's near,
/// far, field of view and aspect, and its eye and interest mirrored below the surface; render
/// links 0, 2 and 5 in camera pass 1, then 4 to 6 and 7 in pass 0 (7 again without fog), onto
/// a white screen without lights; copied into the image, with the texture matrix that projects
/// it onto the water; and the fighters' own callback back (`ftDrawCommon_80081118`).
#[doc(alias = "grIzumi_801CCEA0")]
pub fn render_reflection(r: &mut Running, gobj: GObjId) {
    use super::super::state::Obj;
    use crate::hsd::{cobj, display, lobj};
    if r.game
        .gr
        .izumi
        .reflection
        .as_ref()
        .and_then(|v| v.image)
        .is_none()
    {
        return;
    }
    let Some(Obj::CObj(c)) = r.game.world.gobj(gobj).obj else {
        panic!("the reflection camera holds no camera");
    };
    fighters_draw_magnified(r.game);
    let main = super::super::camera::gobj(&r.game.camera.cam);
    if let Some(main) = main {
        super::super::camera::update_cobj(r.game, main);
    }
    if let Some(Obj::CObj(src)) = main.and_then(|m| r.game.world.gobj(m).obj) {
        let o = &mut r.game.objs;
        let near = cobj::near(&o.cobjs, Some(src));
        cobj::set_near(&mut o.cobjs, Some(c), near);
        let far = cobj::far(&o.cobjs, Some(src));
        cobj::set_far(&mut o.cobjs, Some(c), far);
        let fov = cobj::fov(&o.cobjs, Some(src));
        cobj::set_fov(&mut o.cobjs, Some(c), fov);
        let aspect = cobj::aspect(&o.cobjs, Some(src));
        cobj::set_aspect(&mut o.cobjs, Some(c), aspect);
        let mut eye = cobj::eye_position(o, r.data, src).expect("the game camera's eye");
        eye.y = mem(fp::fneg(reg(eye.y)));
        cobj::set_eye_position(o, c, eye);
        let mut at = cobj::interest(o, r.data, src).expect("the game camera's interest");
        at.y = mem(fp::fneg(reg(at.y)));
        cobj::set_interest(o, c, at);
    }
    if display::camera_begin(&mut r.game.objs, r.data, Some(c)) {
        let draws = &mut r.game.objs.display.draws;
        draws.push(display::Draw::EraseColor([0xFF, 0xFF, 0xFF, 1]));
        draws.push(display::Draw::Erase);
        lobj::delete_current_all(&mut r.game.objs, None);
        super::super::camera::set_layer(&mut r.game.camera.cam, 0);
        super::super::camera::set_pass(&mut r.game.camera.cam, 1);
        r.game.world.gobj_mut(gobj).gxlink_prios = 0x25;
        r.render_links(gobj, 3);
        super::super::camera::set_pass(&mut r.game.camera.cam, 0);
        r.game.world.gobj_mut(gobj).gxlink_prios = 0x70;
        r.render_links(gobj, 7);
        display::fog_set(&mut r.game.objs, None);
        r.game.world.gobj_mut(gobj).gxlink_prios = 0x80;
        r.render_links(gobj, 7);
        display::camera_end(&mut r.game.objs);
    }
    r.game
        .objs
        .display
        .draws
        .push(display::Draw::ReflectionCopy);
    let o = &mut r.game.objs;
    let (fov, aspect) = (
        cobj::fov(&o.cobjs, Some(c)),
        cobj::aspect(&o.cobjs, Some(c)),
    );
    let proj = crate::mtx::Mtx34::light_perspective(fov, aspect, 0.49, -0.49, 0.5, 0.5);
    let view = cobj::viewing_mtx(o, r.data, c);
    let m = proj.concat(&view);
    if let Some(v) = r.game.gr.izumi.reflection.as_mut() {
        v.texture_matrix = Some(m);
    }
    fighters_draw_normally(r.game);
}

/// Every fighter's render callback, in the fighter link's order.
fn fighters_render_with(game: &mut Game, cb: u32) {
    let mut f = game.world.first_in(8);
    while let Some(g) = f {
        game.world.gobj_mut(g).render = Some(CodeRef(cb));
        f = game.world.gobj(g).next;
    }
}

/// The fighters drawn as into the magnifier (`ftDrawCommon_80080C28`), `ftDrawCommon_80081140`.
#[doc(alias = "ftDrawCommon_80081140")]
pub fn fighters_draw_magnified(game: &mut Game) {
    fighters_render_with(game, 0x8008_0C28);
}

/// The fighters' own render callback back (`ftDrawCommon_80080E18`), `ftDrawCommon_80081118`.
#[doc(alias = "ftDrawCommon_80081118")]
pub fn fighters_draw_normally(game: &mut Game) {
    fighters_render_with(game, 0x8008_0E18);
}

/// The stage's shadow check, `grIzumi_801CD280`: a fighter casts on a joint below its feet.
#[doc(alias = "grIzumi_801CD280")]
pub fn check_shadow_render(
    game: &mut Game,
    data: &melee_data::archive::Archives,
    pos: Vec3,
    j: Option<JObjId>,
) -> bool {
    let b = crate::lb::joint::world_position(&mut game.objs, data, j, None);
    reg(pos.y) > reg(b.y)
}
