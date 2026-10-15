//! Pokémon Stadium, `melee/gr/grpstadium.c`: the default stage (map 5), the big screen (map 1)
//! and the transformations (map 2's cycle): every so often the stage sinks and one of the
//! fire, grass, water and rock stages (maps 3, 4, 9 and 6, each from its own file, `GrPs1.dat`
//! to `GrPs4.dat`) rises in its place, and later the default stage comes back.
//!
//! The big screen shows text through a camera of its own rendering into a texture (the
//! players' names and the clock, a KO, the transformation's announcement), the stage seen
//! through the game camera, or a zoomed view of a player: each is a GObj on the max render link
//! whose render callback copies the frame into the screen's image descriptor, and marks the
//! display (`rendered`) for the display's own render callback to switch its texture over.
//!
//! The Slippi ruleset's codes for the stage (`docs/oracle.md`) change four things here: the
//! transformation file loads synchronously inside the tick (`lbFile_80016580`, `800165AC`), so
//! the cycle's wait ends as soon as its file is registered (`801D4760`); a transformation's
//! start leaves the random seed alone when the replay says so (`801D457C`); the archive table's
//! entries free as the codes free them (`grAnime_801C65B0`, `801C65C8`); and whether the zoomed
//! view keeps its player tests fixed bounds instead of the camera's view (`801D24FC`). Slippi
//! 3.16's online codes freeze the stage instead ([`frozen`]): the cycle never runs, and only
//! the zoomed view's code changes anything else.

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{ArchiveId, Archives, DataRef};

use super::super::camera;
use super::super::handles::{CameraBoxId, CodeRef};
use super::super::slippi::CodeSet;
use super::super::state::{Game, OBJ_CAMERA, Obj, Running, UserData};
use super::super::structs::HSD_ImageDesc;
use super::data::{STADIUM_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, map_gobj, nth_joint, scale, update_map_coll};
use super::{StageArchive, anime, grlib, setup, zako};
use crate::fpu::{mem, reg};
use crate::game::hud::magnify::TexBuffer;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::mobj::MObjId;
use crate::hsd::sis::TextId;
use crate::hsd::tobj::{ImageRef, TObjId};
use crate::hsd::{cobj, display, mobj};
use crate::vec::Vec3;

/// The big screen's map id (`PsType_Display`).
pub const MAP_DISPLAY: i32 = 1;
/// The transformations' controller's map id.
pub const MAP_MAIN: i32 = 2;
/// The default stage's map id.
pub const MAP_DEFAULT: i32 = 5;

/// The transformation files, by the cycle's index (`datfiles`): fire, grass, water, rock.
pub const FILES: [&str; 4] = ["GrPs1.dat", "GrPs2.dat", "GrPs3.dat", "GrPs4.dat"];

/// The screens' image descriptors, by the [`Screen`] index the TObj names
/// ([`ImageRef::Screen`]).
pub const SCREEN_TEXT: u8 = 0;
pub const SCREEN_FEED: u8 = 1;
pub const SCREEN_ZOOM: u8 = 2;

/// The stage's state besides its objects' variables.
#[derive(melee_derive::CloneFrom, Debug, Default, melee_derive::StateEq)]
pub struct Globals {
    /// `yakumono_param`, the archive's `grPStadium_YakumonoParam`.
    pub yakumono: Option<DataRef>,
    /// The screens' user data (the wrappers `HSD_MemAlloc` gives), by [`SCREEN_TEXT`] and its
    /// siblings: each GObj's user data is [`UserData::StadiumScreen`] with its index.
    pub screens: [Option<Screen>; 3],
    /// The transformation files the frontend loaded with the match's archives, by
    /// [`FILES`]' order: disc data the load takes rather than reads (an input, like the
    /// fighters' files).
    pub files: [Option<ArchiveId>; 4],
    /// The transformation files' buffer's address (`vs_match::Settings::stadium_buffer`): an
    /// input the match's preload decides.
    pub buffer_address: u32,
}

/// A screen's user data: its image descriptor (`ImageDescWrapper` or `TextWrapper`), for the
/// text camera its two texts, for the feeds whether the frame was copied since the display
/// asked (`flag`) and where the zoomed view copies from.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Screen {
    pub desc: HSD_ImageDesc,
    /// `TextWrapper::win_static_p` and `win_dynamic_p`.
    pub win_static: Option<TextId>,
    pub win_dynamic: Option<TextId>,
    /// `ImageDescWrapper::flag`, `x1A` and `x1C`.
    pub flag: bool,
    pub copy_x: u16,
    pub copy_y: u16,
    /// Where the original's heap put this user data (`docs/engine.md`).
    pub addr: u32,
}

/// A screen's user data from the scene's heap (its block's size), then its image's buffer
/// (`lb_800121FC`): preload entry `entry` where the preload cache holds it (a match the VS mode
/// starts from its menus), else the scene's heap's (the match a replay's playback boots into).
fn alloc_screen(r: &Running, size: u32, desc: &HSD_ImageDesc, entry: i32) -> u32 {
    let addr = crate::hsd::heap::mem_alloc(size);
    if !r.game.ft.data.files.buffer_preloaded(entry) {
        let (w, h) = (u32::from(desc.width), u32::from(desc.height));
        crate::hsd::heap::mem_alloc(
            crate::hsd::heap::tex_buffer_size(w, h, desc.format as u32).next_multiple_of(32),
        );
    }
    addr
}

/// The fields of `grPStadium_YakumonoParam` the code reads, by offset.
#[derive(Clone, Copy, melee_derive::StateEq)]
struct Param(DataRef);

impl Param {
    fn i(self, data: &Archives, off: u32) -> i32 {
        data.u32(self.0.at(off)) as i32
    }

    fn h(self, data: &Archives, off: u32) -> i16 {
        data.u16(self.0.at(off)) as i16
    }

    fn rgb(self, data: &Archives) -> (u8, u8, u8) {
        (
            data.u8(self.0.at(0x1C)),
            data.u8(self.0.at(0x1D)),
            data.u8(self.0.at(0x1E)),
        )
    }
}

fn param(game: &Game) -> Param {
    Param(
        game.gr
            .stadium
            .yakumono
            .expect("grpstadium.c: yakumono_param"),
    )
}

/// `randi`: none drawn for 0.
fn randi(game: &mut Game, max: i32) -> i32 {
    if max != 0 {
        game.objs.rng.below(max)
    } else {
        0
    }
}

/// `randi_between_2`: between the two, whichever is lower, drawn only if they differ.
fn randi_between(game: &mut Game, a: i32, b: i32) -> i32 {
    if b > a {
        a.wrapping_add(randi(game, b.wrapping_sub(a)))
    } else if b < a {
        b.wrapping_add(randi(game, a.wrapping_sub(b)))
    } else {
        b
    }
}

/// A stage object's process or callback, by address: Pokémon Stadium's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x801D_13C4 | 0x801D_156C | 0x801D_1E18 | 0x801D_1E1C => {}
        0x801D_11E4 => start_display(r, gobj),
        0x801D_1290 => init_display(r, gobj),
        0x801D_1390 => proc_display(r, gobj),
        0x801D_13C8 => start_main(r, gobj),
        0x801D_13E0 => init_main(r, gobj),
        0x801D_1520 => proc_main(r, gobj),
        0x801D_1570 => init_default(r, gobj),
        0x801D_1604 | 0x801D_16DC | 0x801D_17E8 | 0x801D_19D8 => {
            update_map_coll(r.game, r.data, gobj);
        }
        0x801D_1624 => r.game.geometry_mut().disable_joint(4),
        0x801D_1648 => init_grass(r, gobj),
        0x801D_16FC => r.game.geometry_mut().disable_joint(3),
        0x801D_1720 => init_rock(r, gobj),
        0x801D_1808 => destroy_rock(r, gobj),
        0x801D_1840 => init_fire(r, gobj),
        0x801D_19F8 => destroy_fire(r, gobj),
        0x801D_1A38 => init_water(r, gobj),
        0x801D_1B48 => proc_water(r, gobj),
        0x801D_1D84 => destroy_water(r, gobj),
        0x801D_1DE4 => {
            let map_id = r.game.grounds[ground_of(r.game, gobj).0].map_id;
            anime::start_map(r.game, r.data, gobj, map_id, 0);
        }
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x801D_101C => on_init(r),
        0x801D_10C8 => on_load(r),
        0x801D_10CC => on_start(r),
        _ => return false,
    }
    true
}

/// The screens' render callbacks, by address.
pub fn run_render(r: &mut Running, gobj: GObjId, cb: u32, pass: i32) -> bool {
    match cb {
        0x801D_5074 => render_display(r, gobj, pass),
        0x801D_2ED0 => render_text_camera(r, gobj),
        0x801D_2FD0 => render_feed(r, gobj),
        0x801D_3084 => render_zoom(r, gobj),
        _ => return false,
    }
    true
}

/// Where the default stage's, the pieces' and a transformation's calls put their scale
/// joints' descriptors on the stack (`setup::SETUP_FRAME`).
const DEFAULT_FRAME: u32 = 0x804E_E9B0;
const PIECES_FRAME: u32 = 0x804E_E910;
const NEXT_FRAME: u32 = 0x804E_E960;

/// Creates one of the stage's objects and sets it up, `grStadium_801D10F8`.
#[doc(alias = "grStadium_801D10F8")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grPs_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &STADIUM_CALLBACKS[map_id as usize]
}

/// Pokémon Stadium's setup, `grStadium_OnInit`: its parameters, map 0, the big screen and the
/// transformations' controller (which makes the default stage), camera limits and blast zones,
/// and the collision joints switched off but the default stage's.
#[doc(alias = "grStadium_OnInit")]
pub fn on_init(r: &mut Running) {
    r.game.gr.stadium.yakumono = r.game.stage_info.yakumono_param;
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    create(r, 0, setup::SETUP_FRAME);
    create(r, MAP_DISPLAY, setup::SETUP_FRAME);
    create(r, MAP_MAIN, setup::SETUP_FRAME);
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    let g = r.game.geometry_mut();
    for j in [1, 2, 3, 5, 7, 0] {
        g.disable_joint(j);
    }
    g.link_joints(6, 4);
}

/// `grStadium_OnDemoInit` does nothing.
#[doc(alias = "grStadium_OnDemoInit")]
pub fn on_demo_init(_r: &mut Running) {}

/// `grStadium_OnLoad` does nothing; `grStadium_801D10F0` (`callback4`) answers false and
/// nothing calls it.
#[doc(alias = "grStadium_OnLoad")]
pub fn on_load(_r: &mut Running) {}

/// Pokémon Stadium's start, `grStadium_OnStart`: the enemy spawner, without enemies.
#[doc(alias = "grStadium_OnStart")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

/// The big screen's start hook, `fn_801D11E4`: a camera subject that frames the stage's
/// middle, off until a transformation is announced.
#[doc(alias = "fn_801D11E4")]
pub fn start_display(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let b = camera::alloc_box(&mut r.game.camera.boxes, camera::ACTIVE);
    r.game.grounds[g.0].u.stadium_display().subject = Some(b);
    let s = reg(scale(r.game));
    let m = |v: f64| mem(fp::fmuls(v, s));
    let o = &mut r.game.camera.boxes[b];
    o.state = camera::INACTIVE;
    o.pos.x = 0.0;
    o.pos.y = m(30.0);
    o.pos.z = 0.0;
    o.target_ext.h.x = m(-25.0);
    o.target_ext.h.y = m(25.0);
    o.target_ext.v.x = m(10.0);
    o.target_ext.v.y = m(-10.0);
}

/// The big screen's setup (map 1), `grStadium_801D1290`: its animation looping, the texture
/// that shows the screens and its material, the screens' GObjs, its start hook, and its own
/// render callback.
#[doc(alias = "grStadium_801D1290")]
pub fn init_display(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let map_id = r.game.grounds[g.0].map_id;
    anime::start_map(r.game, r.data, gobj, map_id, 0);
    anime::set_loop(r.game, gobj, 0, 7, true);
    let head = super::datfiles::archive_for(r.game, r.data, 1).expect("grDatFiles_801C6330(1)");
    let archive = head.0.archive;
    let image = r
        .data
        .get(archive)
        .public("GrdPStadiumBG_OVDummy_mat6962_GrdPStadiumDummy_0_image_desc")
        .map(|offset| DataRef { archive, offset });
    assert!(image.is_some(), "no such image desc!");
    let found = find_tobj(r.game, gobj, image.map(ImageRef::Data));
    assert!(found.is_some(), "no such tobj!");
    let v = r.game.grounds[g.0].u.stadium_display();
    v.image = image;
    // The original leaves its stack slot as it was when none is found; one always is.
    if let Some((t, m)) = found {
        v.tobj = Some(t);
        v.mobj = Some(m);
    }
    reset_display(r, gobj);
    setup::add_start_hook(r.game, gobj, CodeRef(0x801D_11E4));
    r.game.grounds[g.0].x11_flags.b012 = 1;
    r.game.world.gobj_mut(gobj).render = Some(CodeRef(0x801D_5074));
}

/// The big screen's process, `grStadium_801D1390`: a rare spark in the crowd, then the
/// screen's mode.
#[doc(alias = "grStadium_801D1390")]
pub fn proc_display(r: &mut Running, gobj: GObjId) {
    crowd_spark(r.game, r.data);
    display_tick(r, gobj);
}

/// The transformations' start hook, `fn_801D13C8`: the cycle runs from the match's start.
#[doc(alias = "fn_801D13C8")]
pub fn start_main(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    r.game.grounds[g.0].u.stadium().bits.b0 = false;
}

/// The transformations' controller's setup (map 2), `grStadium_801D13E0`: its collision and
/// animation, the transformation files' buffer, the default stage, the first wait, the two
/// lines the transformations' edges add switched off, and its start hook.
#[doc(alias = "grStadium_801D13E0")]
pub fn init_main(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let map_id = r.game.grounds[g.0].map_id;
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, map_id);
    anime::start_map(r.game, r.data, gobj, map_id, 0);
    {
        // `lbDvd_GetPreloadedArchive(0x7D5)`: the preload cache's buffer where it holds one
        // (a match the VS mode starts from its menus), placed outside the simulation, so the
        // address the frontend gives; in the match a replay's playback boots into it holds
        // none, and the scene's heap gives the buffer (the allocator model's address, or
        // without the model the frontend's).
        let address = if r.game.ft.data.files.buffer_preloaded(0x7D5) {
            r.game.gr.stadium.buffer_address
        } else {
            crate::hsd::heap::with(|h| h.mem_alloc(0x5_0000))
                .unwrap_or(r.game.gr.stadium.buffer_address)
        };
        let v = r.game.grounds[g.0].u.stadium();
        v.buffer = address;
        v.bits.b1 = false;
        v.archive = None;
        v.path_len = 0.0;
        v.state = 0;
        v.kind = MAP_DEFAULT as i16;
        v.prev = -1;
        v.prev2 = -1;
    }
    let default = create(r, MAP_DEFAULT, DEFAULT_FRAME);
    let p = param(r.game);
    let (lo, hi) = (p.i(r.data, 0), p.i(r.data, 4));
    let timer = if hi > lo {
        lo.wrapping_add(randi(r.game, hi.wrapping_sub(lo)))
    } else if hi < lo {
        hi.wrapping_add(randi(r.game, lo.wrapping_sub(hi)))
    } else {
        hi
    };
    let v = r.game.grounds[g.0].u.stadium();
    v.current = default;
    v.next = None;
    v.timer = timer;
    if frozen(r.game) {
        // Slippi 3.16's `Init isLoaded Bool` (`801D14C8`): no preloaded transformation's load
        // is out.
        r.game.grounds[g.0].u.stadium().preload_out = 0;
    }
    r.game.grounds[g.0].x11_flags.b012 = 1;
    let geo = r.game.geometry_mut();
    geo.disable_line(0x55);
    geo.disable_line(0x6F);
    setup::add_start_hook(r.game, gobj, CodeRef(0x801D_13C8));
    r.game.grounds[g.0].u.stadium().bits.b0 = true;
}

/// The transformations' controller's process, `grStadium_801D1520`: the cycle once the match
/// has started, then the wind and the collision.
#[doc(alias = "grStadium_801D1520")]
pub fn proc_main(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    if !r.game.grounds[g.0].u.stadium().bits.b0 {
        cycle(r, gobj);
    }
    // The inline `Ground_UpdateWindAndMapColl`.
    super::ground::quake_list_tick(r.game);
    update_map_coll(r.game, r.data, gobj);
}

/// A stage's common setup: its collision on, the given joints listed, its animation, its
/// flags cleared and `x10_flags.b5` set.
fn init_piece(r: &mut Running, gobj: GObjId, joints: &[i32]) {
    let g = ground_of(r.game, gobj);
    let map_id = r.game.grounds[g.0].map_id;
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, map_id);
    setup::switch_on(r.game, r.data, map_id);
    for &k in joints {
        r.game.geometry_mut().enable_joint(k);
    }
    anime::start_map(r.game, r.data, gobj, map_id, 0);
}

/// The default stage's setup (map 5), `grStadium_801D1570`.
#[doc(alias = "grStadium_801D1570")]
pub fn init_default(r: &mut Running, gobj: GObjId) {
    init_piece(r, gobj, &[4]);
    let g = ground_of(r.game, gobj);
    *r.game.grounds[g.0].u.stadium_bits() = Default::default();
    r.game.grounds[g.0].x10_flags.b5 = true;
}

/// The grass stage's setup (map 4), `grStadium_801D1648`.
// Ports `grStadium_801D1648`; aliased once a check reaches it.
pub fn init_grass(r: &mut Running, gobj: GObjId) {
    init_piece(r, gobj, &[3]);
    let g = ground_of(r.game, gobj);
    *r.game.grounds[g.0].u.stadium_bits() = Default::default();
    r.game.grounds[g.0].x10_flags.b5 = true;
}

/// The rock stage's setup (map 6), `grStadium_801D1720`: also its dust's generator.
#[doc(alias = "grStadium_801D1720")]
pub fn init_rock(r: &mut Running, gobj: GObjId) {
    init_piece(r, gobj, &[5]);
    let g = ground_of(r.game, gobj);
    r.game.grounds[g.0].u.stadium_piece().bits = Default::default();
    let generator = grlib::generator_at(r.game, r.data, 0x7546, 0x1E, Vec3::default());
    r.game.grounds[g.0].u.stadium_piece().generator = generator;
    r.game.grounds[g.0].x10_flags.b5 = true;
}

/// The rock stage's destruction, `grStadium_801D1808`: its generator removed, its joint off.
#[doc(alias = "grStadium_801D1808")]
pub fn destroy_rock(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    if let Some(generator) = r.game.grounds[g.0].u.stadium_piece().generator {
        grlib::kill_generator(r.game, r.data, generator);
    }
    r.game.geometry_mut().disable_joint(5);
}

/// The fire stage's setup (map 3), `grStadium_801D1840`: its collision on and its flames'
/// generators on seven joints.
#[doc(alias = "grStadium_801D1840")]
pub fn init_fire(r: &mut Running, gobj: GObjId) {
    init_piece(r, gobj, &[1, 2]);
    let g = ground_of(r.game, gobj);
    *r.game.grounds[g.0].u.stadium_bits() = Default::default();
    for (part, idx) in [
        (0x13, 0x753F),
        (0x14, 0x7549),
        (0x15, 0x7549),
        (0x16, 0x753F),
        (0x17, 0x753F),
        (0x18, 0x7549),
        (0x19, 0x753F),
    ] {
        if let Some(j) = nth_joint(r.game, gobj, part) {
            grlib::generator_on(r.game, r.data, idx, 0x1E, Some(j));
        }
    }
    r.game.grounds[g.0].x10_flags.b5 = true;
}

/// The fire stage's destruction, `grStadium_801D19F8`: part 18's generators and particles
/// removed, its joints off.
#[doc(alias = "grStadium_801D19F8")]
pub fn destroy_fire(r: &mut Running, gobj: GObjId) {
    if let Some(j) = nth_joint(r.game, gobj, 0x12) {
        grlib::kill_tree_generators_and_particles(r.game, r.data, Some(j));
    }
    let geo = r.game.geometry_mut();
    geo.disable_joint(1);
    geo.disable_joint(2);
}

/// The water stage's setup (map 9), `grStadium_801D1A38`: its collision, the windmill's blades
/// (maps 7 and 8), map 0's joint off, its generator, the joint that turns, and every joint's
/// flag 8.
#[doc(alias = "grStadium_801D1A38")]
pub fn init_water(r: &mut Running, gobj: GObjId) {
    init_piece(r, gobj, &[7]);
    let g = ground_of(r.game, gobj);
    let a = create(r, 7, PIECES_FRAME);
    let b = create(r, 8, PIECES_FRAME);
    r.game.grounds[g.0].u.stadium_piece().blades = [a, b];
    r.game.geometry_mut().disable_joint(0);
    r.game.grounds[g.0].u.stadium_piece().bits = Default::default();
    let generator = grlib::generator_at(r.game, r.data, 0x7544, 0x1E, Vec3::default());
    let turning = nth_joint(r.game, gobj, 7);
    let v = r.game.grounds[g.0].u.stadium_piece();
    v.generator = generator;
    v.turning = turning;
    let j = jobj_of(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, 8);
    r.game.grounds[g.0].x10_flags.b5 = true;
}

/// The water stage's process, `grStadium_801D1B48`: the blades follow its height, the windmill
/// turns, map 0's joint comes on or goes off as asked, and the collision follows.
#[doc(alias = "grStadium_801D1B48")]
pub fn proc_water(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("HSD_JObjGetScaleY: jobj");
    let sy = r.game.objs.jobjs[j].scale.y;
    let v = r.game.grounds[g.0].u.stadium_piece();
    let (blades, turning) = (v.blades, v.turning);
    for blade in blades.into_iter().flatten() {
        let bj = jobj_of(r.game, blade).expect("HSD_JObjSetScaleY: jobj");
        jobj::set_srt(&mut r.game.objs.jobjs, bj, |o| o.scale.y = sy);
    }
    let t = turning.expect("HSD_JObjAddRotationZ: jobj");
    jobj::set_srt(&mut r.game.objs.jobjs, t, |o| {
        o.rotate.z = mem(fp::fadds(
            reg(o.rotate.z),
            fp::fmuls(fp::fneg(0.5), 0.017_453_292_384_743_69),
        ));
    });
    let v = r.game.grounds[g.0].u.stadium_piece();
    if v.bits.b0 {
        v.bits.b0 = false;
        r.game.geometry_mut().enable_joint(0);
    }
    let v = r.game.grounds[g.0].u.stadium_piece();
    if v.bits.b1 {
        v.bits.b1 = false;
        r.game.geometry_mut().disable_joint(0);
    }
    update_map_coll(r.game, r.data, gobj);
    r.game.geometry_mut().update_joint_islands(0);
}

/// The water stage's destruction, `grStadium_801D1D84`: its generator and particles, the
/// blades, and its joint off.
#[doc(alias = "grStadium_801D1D84")]
pub fn destroy_water(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.stadium_piece();
    let (generator, blades) = (v.generator, v.blades);
    if let Some(generator) = generator {
        grlib::kill_generator_and_particles(r.game, r.data, generator);
    }
    for b in blades.into_iter().flatten() {
        setup::destroy(r, Some(b));
    }
    r.game.geometry_mut().disable_joint(7);
}

/// A spark in the crowd one tick in 200, `grStadium_801D1E20`: left or right of the stage, 100
/// to 300 units out.
#[doc(alias = "grStadium_801D1E20")]
pub fn crowd_spark(game: &mut Game, data: &Archives) {
    if game.objs.rng.below(200) != 0 {
        return;
    }
    let a = game.objs.rng.below(2);
    let b = game.objs.rng.below(200);
    let mut x = 0.0f32;
    let off = (a * 2 - 1).wrapping_mul(b + 100);
    x = mem(fp::fadds(reg(x), fp::frsp(f64::from(off))));
    let s = reg(scale(game));
    let m = |v: f32| mem(fp::fmuls(reg(v), s));
    let pos = Vec3::new(m(x), m(-100.0), m(-660.0));
    grlib::generator_at(game, data, 0x7530, 0x1E, pos);
}

/// The texture and material in a stage object's model whose image is `image`, depth first
/// (`grStadium_801D3138`).
#[doc(alias = "grStadium_801D3138")]
pub fn find_tobj(game: &Game, gobj: GObjId, image: Option<ImageRef>) -> Option<(TObjId, MObjId)> {
    let objs = &game.objs;
    let mut cur = jobj_of(game, gobj);
    while let Some(j) = cur {
        let o = &objs.jobjs[j];
        if o.flags & (jobj::SPLINE | jobj::PTCL) == 0 {
            let mut d = o.dobj();
            while let Some(dd) = d {
                if let Some(m) = objs.dobjs[dd].mobj {
                    let mut t = objs.mobjs[m].tobj;
                    while let Some(tt) = t {
                        if objs.tobjs[tt].imagedesc == image {
                            return Some((tt, m));
                        }
                        t = objs.tobjs[tt].next;
                    }
                }
                d = objs.dobjs[dd].next;
            }
        }
        cur = if o.flags & jobj::INSTANCE == 0 && o.child.is_some() {
            o.child
        } else if o.next.is_some() {
            o.next
        } else {
            let mut up = j;
            loop {
                match objs.jobjs[up].parent {
                    None => break None,
                    Some(p) => {
                        if let Some(n) = objs.jobjs[p].next {
                            break Some(n);
                        }
                        up = p;
                    }
                }
            }
        };
    }
    None
}

// The big screen.

/// The screen's mode table (`grStadium_801D2528`'s cases): 0 blank, 1 the names and clock, 2
/// to 6 a transformation's announcement (the default stage, fire, grass, water, rock), 7 the
/// stage feed, 8 a player zoomed in, 9 a KO, 10 to 13 Ready and GO! and GAME!, 14 the
/// results, 15 and 16 the screen's two pictures, 17 the static text 4.
fn set_mode(r: &mut Running, gobj: GObjId, mode: i32, timer: i32) {
    let mut mode = mode;
    if super::super::gm::is_training(r.game) {
        match mode {
            1 | 9..=16 => mode = 7,
            2..=6 => panic!("the original's assertion failed: grpstadium.c:1334"),
            _ => {}
        }
    }
    let p = param(r.game);
    let g = ground_of(r.game, gobj);
    {
        let v = r.game.grounds[g.0].u.stadium_display();
        v.prev_mode = v.mode;
        v.mode = mode as i16;
    }
    let inactive = |r: &mut Running| {
        let v = r.game.grounds[g.0].u.stadium_display();
        if let Some(b) = v.subject {
            r.game.camera.boxes[b].state = camera::INACTIVE;
        }
    };
    let active = |r: &mut Running| {
        let v = r.game.grounds[g.0].u.stadium_display();
        if let Some(b) = v.subject {
            r.game.camera.boxes[b].state = camera::ACTIVE;
        }
    };
    let quiet = |r: &mut Running, shown: i16| {
        let v = r.game.grounds[g.0].u.stadium_display();
        v.shown = shown;
        v.rendered = false;
        v.xF8_1 = false;
    };
    match mode {
        0 => {
            text_blank(r, gobj);
            quiet(r, 0x40);
            inactive(r);
        }
        1 => {
            text_names_or_count(r, gobj);
            quiet(r, 0x40);
            r.game.grounds[g.0].u.stadium_display().timer = p.i(r.data, 0x20);
            inactive(r);
        }
        9 => {
            text_ko(r, gobj);
            quiet(r, 0x40);
            r.game.grounds[g.0].u.stadium_display().timer = p.i(r.data, 0x24);
            inactive(r);
        }
        14 => {
            text_results(r, gobj);
            quiet(r, 0x40);
            r.game.grounds[g.0].u.stadium_display().timer = p.i(r.data, 0x2C);
            inactive(r);
        }
        10..=13 => {
            text_announce(r, gobj, mode);
            quiet(r, 0x40);
            inactive(r);
        }
        15 | 16 => {
            let v = r.game.grounds[g.0].u.stadium_display();
            v.shown = if mode == 15 { 0x20 } else { 0x80 };
            v.timer = p.i(r.data, 0x20);
            inactive(r);
        }
        7 => {
            r.game.grounds[g.0].u.stadium_display().shown = 0x40;
            let t = randi_between(r.game, p.i(r.data, 0x38), p.i(r.data, 0x3C));
            r.game.grounds[g.0].u.stadium_display().timer = t;
            quiet(r, 0x40);
            inactive(r);
        }
        8 => {
            r.game.grounds[g.0].u.stadium_display().shown = 0x40;
            let t = randi_between(r.game, p.i(r.data, 0x30), p.i(r.data, 0x34));
            r.game.grounds[g.0].u.stadium_display().timer = t;
            // The next player with a fighter, at most three times round.
            let v = r.game.grounds[g.0].u.stadium_display();
            let n = v.player;
            v.player = v.player.wrapping_add(1);
            let mut i = 0;
            loop {
                let v = r.game.grounds[g.0].u.stadium_display();
                if v.player == n {
                    break;
                }
                if v.player >= 6 {
                    v.player = 0;
                    i += 1;
                    if i > 2 {
                        v.player = 0;
                        v.timer = -1;
                        break;
                    }
                }
                let slot = i32::from(v.player);
                if r.game.player_entity(slot).is_some() {
                    break;
                }
                let v = r.game.grounds[g.0].u.stadium_display();
                v.player = v.player.wrapping_add(1);
            }
            quiet(r, 0x40);
            inactive(r);
        }
        17 => {
            r.game.grounds[g.0].u.stadium_display().shown = 0x40;
            let screen = r.game.gr.stadium.screens[SCREEN_TEXT as usize]
                .as_ref()
                .expect("the text screen");
            let (st, dy) = (screen.win_static, screen.win_dynamic);
            r.game.sis.clear_text(dy.expect("win_dynamic_p"));
            r.game.sis.reset_text(r.data, st.expect("win_static_p"), 4);
            quiet(r, 0x40);
        }
        2..=6 => {
            let shown = [1, 2, 4, 0x10, 8][(mode - 2) as usize];
            let v = r.game.grounds[g.0].u.stadium_display();
            v.shown = shown;
            v.timer = p.i(r.data, 0x28);
            active(r);
        }
        _ => {}
    }
    if timer != 0 {
        r.game.grounds[g.0].u.stadium_display().timer = timer;
    }
}

/// [`set_mode`], `grStadium_801D2528`.
#[doc(alias = "grStadium_801D2528")]
pub fn show(r: &mut Running, gobj: GObjId, mode: i32, timer: i32) {
    set_mode(r, gobj, mode, timer);
}

/// The display's first state, `grStadium_801D2278`: the names and clock, nothing applied,
/// no player focused, the screens' GObjs (the zoomed view first copied from the middle of the
/// screen), and the blank mode.
#[doc(alias = "grStadium_801D2278")]
pub fn reset_display(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    {
        let v = r.game.grounds[g.0].u.stadium_display();
        v.mode = 1;
        v.shown = 1;
        v.applied = -1;
        v.rendered = false;
        v.xF8_1 = false;
        v.xF8_2 = false;
        v.prev_mode = -1;
        v.player = 99;
        v.count = 0;
    }
    let text = create_text_camera(r);
    let feed = create_feed(r);
    let zoom = create_zoom(r);
    let v = r.game.grounds[g.0].u.stadium_display();
    v.text = Some(text);
    v.feed = Some(feed);
    v.zoom = Some(zoom);
    let z = r.game.gr.stadium.screens[SCREEN_ZOOM as usize]
        .as_mut()
        .expect("the zoomed view's user data");
    z.copy_x = 200;
    z.copy_y = 160;
    r.game.grounds[g.0].u.stadium_display().subject = None;
    set_mode(r, gobj, 0, 0);
}

/// The display's tick, `grStadium_801D2344`: the text modes rebuild their text; a mode whose
/// time runs out picks the next ([`next_mode`]); the feeds ask for a new frame; a zoomed view
/// ends when its player is gone or leaves the view.
#[doc(alias = "grStadium_801D2344")]
pub fn display_tick(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let mode = r.game.grounds[g.0].u.stadium_display().mode;
    let timer_out = |r: &mut Running| {
        let v = r.game.grounds[g.0].u.stadium_display();
        let t = v.timer;
        v.timer = t.wrapping_sub(1);
        t < 0
    };
    match mode {
        0 => text_blank(r, gobj),
        1 => {
            text_names_or_count(r, gobj);
            if timer_out(r) {
                next_mode(r, gobj);
            }
        }
        9 => {
            text_ko(r, gobj);
            if timer_out(r) {
                next_mode(r, gobj);
            }
        }
        14 => {
            text_results(r, gobj);
            if timer_out(r) {
                next_mode(r, gobj);
            }
        }
        10..=13 => text_announce(r, gobj, i32::from(mode)),
        2..=6 | 15 | 16 => {
            if timer_out(r) {
                next_mode(r, gobj);
            }
        }
        7 => {
            if timer_out(r) {
                next_mode(r, gobj);
                return;
            }
            screen_mut(r.game, SCREEN_FEED).flag = false;
        }
        8 => {
            let player = i32::from(r.game.grounds[g.0].u.stadium_display().player);
            let gone = |r: &Running| {
                r.game.player_entity(player).is_none() || r.game.player_asleep(player)
            };
            if timer_out(r) || gone(r) {
                next_mode(r, gobj);
                return;
            }
            screen_mut(r.game, SCREEN_ZOOM).flag = false;
            if gone(r) || !zoom_keeps_player(r, gobj) {
                next_mode(r, gobj);
            }
        }
        _ => {}
    }
}

/// Whether the zoomed view keeps its player: the original asks whether the player stays inside
/// the camera's view (`grStadium_801D32D0`); the Slippi ruleset's `PSCameraIndependentMonitor`
/// (at `801D24FC`) still runs that for where to copy from, but asks instead whether the
/// player's position is within fixed bounds, the code's own data.
fn zoom_keeps_player(r: &mut Running, gobj: GObjId) -> bool {
    let in_view = zoom_origin(r, gobj);
    if !r.game.rules.slippi {
        return in_view;
    }
    let g = ground_of(r.game, gobj);
    let player = i32::from(r.game.grounds[g.0].u.stadium_display().player);
    let f = r
        .game
        .player_entity(player)
        .expect("the zoomed player's fighter");
    let pos = r.game.fighters[r.game.fighter_id(f).0].cur_pos;
    let [left, right, top, bottom] = MONITOR_BOUNDS.map(f32::from_bits);
    let (x, y) = (reg(pos.x), reg(pos.y));
    !(x < reg(left) || x > reg(right) || y > reg(top) || y < reg(bottom))
}

/// `PSCameraIndependentMonitor`'s bounds (its data words): left -120, right 120, top 80,
/// bottom -20.
pub const MONITOR_BOUNDS: [u32; 4] = [0xC2F0_0000, 0x42F0_0000, 0x42A0_0000, 0xC1A0_0000];

/// Where the zoomed view copies from, around its player as the game camera sees them, kept on
/// the camera's viewport; whether the player was inside it, `grStadium_801D32D0`.
#[doc(alias = "grStadium_801D32D0")]
pub fn zoom_origin(r: &mut Running, gobj: GObjId) -> bool {
    let g = ground_of(r.game, gobj);
    let Some(cam) = camera::gobj(&r.game.camera.cam) else {
        return false;
    };
    let Some(Obj::CObj(c)) = r.game.world.gobj(cam).obj else {
        return false;
    };
    let player = i32::from(r.game.grounds[g.0].u.stadium_display().player);
    let Some(f) = r.game.player_entity(player) else {
        return false;
    };
    if r.game.player_asleep(player) {
        return false;
    }
    let fid = r.game.fighter_id(f);
    let b = r.game.fighters[fid.0]
        .x890_cameraBox
        .expect("ftLib_GetCameraSubject: a fighter without a camera box");
    let bone = r.game.camera.boxes[b].bone_pos;
    let s = crate::lb::screen::world_to_screen(&mut r.game.objs, r.data, c, bone, false)
        .expect("lbVector_WorldToScreen: a frustum camera");
    let vp = r.game.objs.cobjs[c].viewport;
    let mut out = false;
    let mut x = fp::fsubs(reg(s.x), 62.0);
    let (xmin, xmax, ymin, ymax) = (
        fp::frsp(f64::from(vp[0])),
        fp::frsp(f64::from(vp[1])),
        fp::frsp(f64::from(vp[2])),
        fp::frsp(f64::from(vp[3])),
    );
    if x < xmin {
        x = xmin;
        out = true;
    } else if fp::fadds(124.0, x) > xmax {
        out = true;
        x = fp::fsubs(xmax, 124.0);
    }
    let mut y = fp::fsubs(reg(s.y), 40.0);
    if y < ymin {
        y = ymin;
        out = true;
    } else if fp::fadds(80.0, y) > ymax {
        out = true;
        y = fp::fsubs(ymax, 80.0);
    }
    let z = screen_mut(r.game, SCREEN_ZOOM);
    z.copy_x = ((fp::fctiwz(x) >> 1).wrapping_mul(2)) as u16;
    z.copy_y = ((fp::fctiwz(y) >> 1).wrapping_mul(2)) as u16;
    !out
}

/// Picks the next mode when one ends, `grStadium_801D2A60`: the results every so many modes,
/// else by the parameters' weights a zoomed player, the stage feed, the names and clock or a
/// picture, never the mode just shown or the one before.
#[doc(alias = "grStadium_801D2A60")]
pub fn next_mode(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let d = r.data;
    let mode = if r.game.grounds[g.0].u.stadium_display().count >= p.h(d, 0x50) {
        r.game.grounds[g.0].u.stadium_display().count = 0;
        14
    } else {
        let (w48, w4a, w4c, w4e) = (
            i32::from(p.h(d, 0x48)),
            i32::from(p.h(d, 0x4A)),
            i32::from(p.h(d, 0x4C)),
            i32::from(p.h(d, 0x4E)),
        );
        let f = |v: i32| fp::frsp(f64::from(v));
        loop {
            let rand = f64::from(r.game.objs.rng.draw_f32());
            let mut val = fp::fmuls(
                f(w48.wrapping_add(w4a).wrapping_add(w4c).wrapping_add(w4e)),
                rand,
            );
            val = fp::fsubs(val, f(w48));
            let m = if val < 0.0 {
                8
            } else {
                val = fp::fsubs(val, f(w4c));
                if val < 0.0 {
                    7
                } else {
                    val = fp::fsubs(val, f(w4a));
                    if val < 0.0 {
                        1
                    } else {
                        val = fp::fsubs(val, f(w4e));
                        if val < 0.0 { 15 } else { 1 }
                    }
                }
            };
            let v = r.game.grounds[g.0].u.stadium_display();
            if m != i32::from(v.mode) && m != i32::from(v.prev_mode) {
                break m;
            }
        }
    };
    let v = r.game.grounds[g.0].u.stadium_display();
    v.count = v.count.wrapping_add(1);
    set_mode(r, gobj, mode, 0);
}

/// A screen's user data.
fn screen_mut(game: &mut Game, which: u8) -> &mut Screen {
    game.gr.stadium.screens[which as usize]
        .as_mut()
        .expect("a screen's user data")
}

/// `grStadium_803E14FC`, the text camera's descriptor (made orthographic after loading).
const TEXT_CAMERA: cobj::StaticDesc = cobj::StaticDesc {
    flags: 0,
    projection_type: 1,
    viewport: [0, 0xFA, 0, 0xA0],
    scissor: [0, 0xFA, 0, 0xA0],
    eye: Vec3::new(0.0, 0.0, 1.0),
    interest: Vec3::new(0.0, 0.0, 0.0),
    roll: 0.0,
    near: 0.1,
    far: 32768.0,
    projection: [30.0, 1.3636, 0.0, 0.0],
};

/// An image descriptor for a screen, `lb_800121FC`: `width` by `height` in `format`, its
/// pixels in the preloaded buffer `entry` names (the match's preload holds every screen's), or
/// else from the heap.
#[doc(alias = "lb_800121FC")]
pub fn screen_image(width: u16, height: u16, format: i32) -> HSD_ImageDesc {
    HSD_ImageDesc {
        image_ptr: TexBuffer(true),
        width,
        height,
        format,
        mipmap: 0,
        minLOD: 0.0,
        maxLOD: 0.0,
    }
}

/// The text camera, `grStadium_801D2BEC`: a GObj (class 0x11, link 0x13) with an orthographic
/// camera onto a 250x160 texture, rendering link 1's texts (render priority 1), its user data
/// the texture's image and two texts of font 1, whose strings come from the stage archive's
/// `SIS_GrPStadiumData`: a static one and one the screen writes into.
#[doc(alias = "grStadium_801D2BEC")]
pub fn create_text_camera(r: &mut Running) -> GObjId {
    let gobj = r.game.world.create(0x11, 0x13, 0);
    let c = crate::lb::cobj::load_cobj(
        &mut r.game.objs,
        r.data,
        crate::lb::cobj::CObjDesc::Static(&TEXT_CAMERA),
    );
    cobj::set_ortho(&mut r.game.objs.cobjs, Some(c), 0.0, -160.0, 0.0, 250.0);
    r.game.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
    r.game
        .world
        .setup_gx_link_max(gobj, CodeRef(0x801D_2ED0), 1, false);
    r.game.world.gobj_mut(gobj).gxlink_prios = 2;
    let desc = screen_image(0xFA, 0xA0, 4);
    // `TextWrapper`.
    let addr = alloc_screen(r, 0x20, &desc, 0x7D2);
    r.game
        .world
        .init_user_data(gobj, 3, UserData::StadiumScreen(SCREEN_TEXT));
    r.game.gr.stadium.screens[SCREEN_TEXT as usize] = Some(Screen {
        desc,
        addr,
        ..Default::default()
    });
    let archive = super::datfiles::main_archive(r.game).archive;
    r.add_canvas(1, Some(gobj), 9, 0xD, 0, 1, 0, 1);
    let table = r
        .data
        .get(archive)
        .public("SIS_GrPStadiumData")
        .map(|offset| DataRef { archive, offset });
    r.game.sis.font_tables[1] = table;
    let st = r.create_text(1, 0, Vec3::new(0.0, 0.0, 0.0), 250.0, 160.0);
    let dy = r.create_text_buffer(1, 0);
    let t = r.game.sis.text_mut(dy);
    t.pos.z = 0.0;
    t.box_size = [250.0, 160.0];
    let s = screen_mut(r.game, SCREEN_TEXT);
    s.win_static = Some(st);
    s.win_dynamic = Some(dy);
    gobj
}

/// The stage feed's GObj, `grStadium_801D2D78`: a 640x406 copy of the screen (render priority
/// 3), copied when the display asks.
#[doc(alias = "grStadium_801D2D78")]
pub fn create_feed(r: &mut Running) -> GObjId {
    let gobj = r.game.world.create(0x11, 0x12, 0);
    r.game
        .world
        .setup_gx_link_max(gobj, CodeRef(0x801D_2FD0), 3, false);
    let desc = screen_image(0x280, 0x196, 4);
    // 0x1C bytes, two short of an `ImageDescWrapper`.
    let addr = alloc_screen(r, 0x1C, &desc, 0x7D3);
    r.game
        .world
        .init_user_data(gobj, 3, UserData::StadiumScreen(SCREEN_FEED));
    r.game.gr.stadium.screens[SCREEN_FEED as usize] = Some(Screen {
        desc,
        flag: true,
        addr,
        ..Default::default()
    });
    gobj
}

/// The zoomed view's GObj, `grStadium_801D2E24`: a 124x80 copy of the screen (render priority
/// 3), copied when the display asks.
#[doc(alias = "grStadium_801D2E24")]
pub fn create_zoom(r: &mut Running) -> GObjId {
    let gobj = r.game.world.create(0x11, 0x12, 0);
    r.game
        .world
        .setup_gx_link_max(gobj, CodeRef(0x801D_3084), 3, false);
    let desc = screen_image(0x7C, 0x50, 4);
    // `ImageDescWrapper`.
    let addr = alloc_screen(r, 0x20, &desc, 0x7D4);
    r.game
        .world
        .init_user_data(gobj, 3, UserData::StadiumScreen(SCREEN_ZOOM));
    r.game.gr.stadium.screens[SCREEN_ZOOM as usize] = Some(Screen {
        desc,
        flag: false,
        addr,
        ..Default::default()
    });
    gobj
}

/// Marks the display: a screen camera rendered.
fn mark_rendered(game: &mut Game) {
    let g = map_gobj(game, MAP_DISPLAY).expect("the original's assertion failed: vision_gobj");
    let gp = ground_of(game, g);
    game.grounds[gp.0].u.stadium_display().rendered = true;
}

/// The text camera's render callback, `fn_801D2ED0`: its texts rendered onto a black screen,
/// copied into its texture (clearing the screen), and the display marked.
#[doc(alias = "fn_801D2ED0")]
pub fn render_text_camera(r: &mut Running, gobj: GObjId) {
    let Some(Obj::CObj(c)) = r.game.world.gobj(gobj).obj else {
        panic!("the text camera holds no camera");
    };
    if display::camera_begin(&mut r.game.objs, r.data, Some(c)) {
        let draws = &mut r.game.objs.display.draws;
        draws.push(display::Draw::EraseColor([0, 0, 0, 1]));
        draws.push(display::Draw::Erase);
        display::fog_set(&mut r.game.objs, None);
        r.render_links(gobj, 7);
        display::camera_end(&mut r.game.objs);
    }
    r.game.objs.display.draws.push(display::Draw::ScreenCopy {
        screen: SCREEN_TEXT,
        x: 0,
        y: 0,
        clear: true,
    });
    mark_rendered(r.game);
}

/// The stage feed's render callback, `grStadium_801D2FD0`: when asked, the screen from
/// (0, 36) copied and the display marked.
#[doc(alias = "grStadium_801D2FD0")]
pub fn render_feed(r: &mut Running, _gobj: GObjId) {
    let s = screen_mut(r.game, SCREEN_FEED);
    if s.flag {
        return;
    }
    s.flag = true;
    r.game.objs.display.draws.push(display::Draw::ScreenCopy {
        screen: SCREEN_FEED,
        x: 0,
        y: 36,
        clear: false,
    });
    mark_rendered(r.game);
}

/// The zoomed view's render callback, `grStadium_801D3084`: when asked, the screen around the
/// player copied and the display marked.
#[doc(alias = "grStadium_801D3084")]
pub fn render_zoom(r: &mut Running, _gobj: GObjId) {
    let s = screen_mut(r.game, SCREEN_ZOOM);
    if s.flag {
        return;
    }
    s.flag = true;
    let (x, y) = (s.copy_x, s.copy_y);
    r.game.objs.display.draws.push(display::Draw::ScreenCopy {
        screen: SCREEN_ZOOM,
        x,
        y,
        clear: false,
    });
    mark_rendered(r.game);
}

/// The big screen's render callback, `fn_801D5074`: its joints and texture switched over
/// ([`apply_display`]), then drawn as any stage object is (`grDisplay_801C5DB0`).
#[doc(alias = "fn_801D5074")]
pub fn render_display(r: &mut Running, gobj: GObjId, pass: i32) {
    apply_display(r, gobj);
    r.render_stage_object(gobj, pass);
}

/// The display's joints for each bit of `shown`, `grStadium_801D1EF8`'s order.
const SHOWN_JOINTS: [(i16, i32); 8] = [
    (1, 6),
    (2, 3),
    (8, 7),
    (0x10, 9),
    (4, 4),
    (0x20, 8),
    (0x80, 5),
    (0x40, 2),
];

/// Once a screen camera has rendered, shows the joints `shown` names that changed and points
/// the screen's texture at the mode's image, `grStadium_801D1EF8`.
#[doc(alias = "grStadium_801D1EF8")]
pub fn apply_display(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.stadium_display();
    if !v.rendered {
        return;
    }
    let (shown, applied, mode, tobj) = (v.shown, v.applied, v.mode, v.tobj);
    let changed = (applied ^ shown) as u32;
    for (bit, part) in SHOWN_JOINTS {
        if changed & bit as u32 == 0 {
            continue;
        }
        if let Some(j) = nth_joint(r.game, gobj, part) {
            if shown & bit != 0 {
                jobj::clear_flags(&mut r.game.objs.jobjs, Some(j), jobj::HIDDEN);
            } else {
                jobj::set_flags(&mut r.game.objs.jobjs, Some(j), jobj::HIDDEN);
            }
        }
    }
    let screen = match mode {
        0 | 1 | 9..=14 | 17 => Some((SCREEN_TEXT, false)),
        7 => Some((SCREEN_FEED, true)),
        8 => Some((SCREEN_ZOOM, true)),
        _ => None,
    };
    if let (Some((which, tint)), Some(t)) = (screen, tobj) {
        let has = r.game.gr.stadium.screens[which as usize].is_some();
        if has {
            r.game.objs.tobjs[t].imagedesc = Some(ImageRef::Screen(which));
            tint_display(r, gobj, tint);
        }
    }
    let v = r.game.grounds[g.0].u.stadium_display();
    v.applied = v.shown;
}

/// The screen's material for a feed (lit, the parameters' diffuse color) or the text (its
/// texture's own colors), `grStadium_801D21E4`.
#[doc(alias = "grStadium_801D21E4")]
pub fn tint_display(r: &mut Running, gobj: GObjId, feed: bool) {
    let g = ground_of(r.game, gobj);
    let Some(m) = r.game.grounds[g.0].u.stadium_display().mobj else {
        return;
    };
    let mobjs = &mut r.game.objs.mobjs;
    if feed {
        mobj::clear_flags(mobjs, Some(m), 2);
        mobj::set_flags(mobjs, Some(m), 1);
        let (cr, cg, cb) = param(r.game).rgb(r.data);
        mobj::set_diffuse_color(&mut r.game.objs.mobjs, m, cr, cg, cb);
    } else {
        mobj::set_flags(mobjs, Some(m), 2);
        mobj::clear_flags(mobjs, Some(m), 1);
    }
    mobj::compile_tev(&mut r.game.objs, Some(m));
}

/// Whether the screen is zoomed in on a player, `grStadium_801D4FF8`.
#[doc(alias = "grStadium_801D4FF8")]
pub fn zoomed_on(game: &mut Game, slot: i32) -> bool {
    if game.stage_info.grkind != super::GR_KIND_PSTADIUM {
        return false;
    }
    let Some(g) = map_gobj(game, MAP_DISPLAY) else {
        return false;
    };
    let gp = ground_of(game, g);
    let v = game.grounds[gp.0].u.stadium_display();
    v.mode == 8 && i32::from(v.player) == slot
}

// The screen's text.

/// The text screen's two texts.
fn texts(game: &Game) -> (TextId, TextId) {
    let s = game.gr.stadium.screens[SCREEN_TEXT as usize]
        .as_ref()
        .expect("the text screen");
    (
        s.win_static
            .expect("the original's assertion failed: gp2->win_static_p"),
        s.win_dynamic
            .expect("the original's assertion failed: gp2->win_dynamic_p"),
    )
}

/// The blank screen, `grStadium_801D4194`: the dynamic text cleared, the static text's
/// string 3.
#[doc(alias = "grStadium_801D4194")]
pub fn text_blank(r: &mut Running, _gobj: GObjId) {
    let (st, dy) = texts(r.game);
    r.game.sis.clear_text(dy);
    r.game.sis.reset_text(r.data, st, 3);
}

/// Ready, GO!, GAME! or the like (modes 10 to 13), `grStadium_801D3F40`: the static text's
/// strings 0x12 to 0x15.
#[doc(alias = "grStadium_801D3F40")]
pub fn text_announce(r: &mut Running, _gobj: GObjId, mode: i32) {
    let (st, dy) = texts(r.game);
    r.game.sis.clear_text(dy);
    let index = match mode {
        10 => 0x12,
        11 => 0x13,
        12 => 0x14,
        13 => 0x15,
        _ => 3,
    };
    r.game.sis.reset_text(r.data, st, index);
}

/// The names and clock, or the count in a mode that counts, `grStadium_801D39A0`.
#[doc(alias = "grStadium_801D39A0")]
pub fn text_names_or_count(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.stadium_display();
    if v.mode != 1 {
        return;
    }
    let x_f8_2 = v.xF8_2;
    if r.game.vs_is_singleplayer() || x_f8_2 {
        super::super::code::unported_call("grStadium_801D384C");
    } else {
        text_names(r, gobj);
    }
}

/// `grPs_8049F040`, the static buffer the names screen builds its string 5 in.
const NAMES_STRING: u32 = 0x8049_F040;

/// Appends a string to the dynamic text at (`x`, `y`): what `vsnprintf` makes of the format
/// and arguments, as `HSD_SisLib_803A6B98` takes them.
fn print(r: &mut Running, t: TextId, x: f32, y: f32, fmt: &[u8], args: &[FmtArg]) {
    let s = crate::hsd::sis::printf(fmt, args);
    r.game
        .sis
        .add_string(t, f64::from(x), f64::from(y), Some(&s));
}

use crate::hsd::sis::{FmtArg, SisPtr};

/// The names and the clock, `grStadium_801D3460`: the static text's string 5 rebuilt in its
/// buffer from string 6, the current stage's name (strings 8 to 12) and string 7, and the
/// players' names placed for how many there are, over the time left (or "00:00 00" without a
/// time limit).
#[doc(alias = "grStadium_801D3460")]
pub fn text_names(r: &mut Running, _gobj: GObjId) {
    let (st, dy) = texts(r.game);
    r.game.sis.clear_text(dy);
    let gm = map_gobj(r.game, MAP_MAIN).expect("the transformations' controller");
    let gp = ground_of(r.game, gm);
    let kind = r.game.grounds[gp.0].u.stadium().kind;
    let s = &mut r.game.sis;
    s.set_table_entry(1, 5, SisPtr::Static(NAMES_STRING));
    s.copy_table_string(r.data, 1, 5, 6);
    let name = match kind {
        3 => 9,
        9 => 0xA,
        6 => 0xB,
        4 => 0xC,
        _ => 8,
    };
    s.append_table_string(r.data, 1, 5, name);
    s.append_table_string(r.data, 1, 5, 7);
    s.reset_text(r.data, st, 5);
    let jp = r.game.gm.unlocks.is_jp();
    let t = r.game.sis.text_mut(dy);
    t.default_kerning = 0;
    t.x34 = if jp { [0.3125, 0.625] } else { [0.26, 0.625] };
    t.text_color = [0xC0, 0xC0, 0xFF, 0xFF];
    t.default_alignment = 1;
    let mut names: Vec<&'static [u8]> = Vec::new();
    for i in 0..4 {
        if r.game.player_pkind(i) != crate::game::pl::player::PKIND_NONE
            && let Some(n) = r.game.player_name(i)
        {
            names.push(n);
        }
    }
    r.game.sis.text_mut(dy).default_fitting = 1;
    let places: &[(f32, f32)] = match names.len() {
        1 => &[(124.0, 70.0)],
        2 => &[(62.0, 70.0), (186.0, 70.0)],
        3 => &[(62.0, 60.0), (186.0, 60.0), (124.0, 80.0)],
        4 => &[(62.0, 60.0), (186.0, 60.0), (62.0, 80.0), (186.0, 80.0)],
        _ => &[],
    };
    for (&(x, y), n) in places.iter().zip(&names) {
        // The name is the format.
        print(r, dy, x, y, n, &[]);
    }
    let t = r.game.sis.text_mut(dy);
    t.default_fitting = 0;
    t.x34 = [0.625, 0.625];
    let start = &r.game.vs.start;
    if start.time_limit == 0 || !start.timer_enabled {
        print(r, dy, 125.0, 120.0, b"00:00 00", &[]);
        return;
    }
    let secs = r.game.vs_timer_seconds() as i32;
    let cs = i32::from(r.game.vs_timer_centiseconds());
    print(
        r,
        dy,
        125.0,
        120.0,
        b"%02d:%02d %02d",
        &[
            FmtArg::Int(secs / 60),
            FmtArg::Int(secs % 60),
            FmtArg::Int(cs),
        ],
    );
}

/// A player's KO, `grStadium_801D3A0C`: the static text's string 3, and "Player N Defeated"
/// for a human, "Computer Player Defeated" for the others.
#[doc(alias = "grStadium_801D3A0C")]
pub fn text_ko(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let (st, dy) = texts(r.game);
    r.game.sis.clear_text(dy);
    r.game.sis.reset_text(r.data, st, 3);
    let v = r.game.grounds[g.0].u.stadium_display();
    let (slot_type, player) = (v.slot_type, v.player);
    let t = r.game.sis.text_mut(dy);
    t.default_kerning = 1;
    t.text_color = [0xC0, 0xC0, 0xFF, 0xFF];
    t.default_alignment = 1;
    if i32::from(slot_type) == crate::game::pl::player::PKIND_HUMAN {
        t.x34 = [0.625, 1.5];
        print(
            r,
            dy,
            125.0,
            56.0,
            b"\x82o\x82\x8c\x82\x81\x82\x99\x82\x85\x82\x92 %d \x82c\x82\x85\x82\x86\x82\x85\x82\x81\x82\x94\x82\x85\x82\x84",
            &[FmtArg::Int(i32::from(player) + 1)],
        );
    } else {
        t.x34 = [0.42, 1.2];
        print(r, dy, 125.0, 56.0, b"Computer Player Defeated", &[]);
    }
}

/// `grPs_803B7F8C`: the first line's height for one to four players left.
const RESULT_TOPS: [u32; 4] = [0x50, 0x40, 0x30, 0x20];

/// A color channel brightened by a quarter, at most 255 (the inline `grStadium_ScaleColor`).
fn brighten(v: u8) -> u8 {
    let mut s = fp::fmuls(1.25, fp::frsp(f64::from(v)));
    if s > 255.0 {
        s = 255.0;
    }
    fp::fctiwz(s) as u8
}

/// The players still in, `grStadium_801D3BBC`: the static text's string for how many (0xE to
/// 0x11, or 3 for none or more than four), and their names, each a line in its player's
/// color brightened.
#[doc(alias = "grStadium_801D3BBC")]
pub fn text_results(r: &mut Running, _gobj: GObjId) {
    let (st, dy) = texts(r.game);
    let in_play = |r: &mut Running, slot: i32| -> bool {
        if r.game.player_pkind(slot) == crate::game::pl::player::PKIND_NONE
            || r.game.player_asleep(slot)
        {
            return false;
        }
        let Some(f) = r.game.player_entity(slot) else {
            return false;
        };
        let idx = i32::from(r.game.fighters[r.game.fighter_id(f).0].player_idx);
        r.game.player_rank(idx) == 0
    };
    let mut n = 0;
    for slot in 0..6 {
        if in_play(r, slot) {
            n += 1;
        }
    }
    r.game.sis.clear_text(dy);
    let index = match n {
        1 => 0xE,
        2 => 0xF,
        3 => 0x10,
        4 => 0x11,
        _ => {
            r.game.sis.reset_text(r.data, st, 3);
            return;
        }
    };
    r.game.sis.reset_text(r.data, st, index);
    let jp = r.game.gm.unlocks.is_jp();
    let t = r.game.sis.text_mut(dy);
    t.default_kerning = 0;
    t.default_alignment = 1;
    t.default_fitting = 1;
    if jp {
        t.x34 = [1.0, 1.0];
    } else {
        t.x34 = [0.8, 1.0];
        t.default_kerning = 1;
    }
    let top = RESULT_TOPS[n - 1] as i32;
    let mut offset = 0;
    for slot in 0..6 {
        if !in_play(r, slot) {
            continue;
        }
        let team = r.game.player_team(slot) as u8;
        let teams = u8::from(r.game.vs_is_teams());
        let pkind = r.game.player_pkind(slot) as u8;
        let c = super::super::gm::vs::hud_color(super::super::gm::vs::player_color(
            slot as u8, team, teams, pkind,
        ));
        r.game.sis.text_mut(dy).text_color = [brighten(c[0]), brighten(c[1]), brighten(c[2]), 0xFF];
        if let Some(name) = r.game.player_name(slot) {
            let y = mem(fp::frsp(f64::from(top + offset)));
            print(r, dy, 125.0, y, b"%s", &[FmtArg::Str(name)]);
        }
        offset += 0x20;
    }
}

// The transformations.

/// The dust's path round the stage (`lbl_803E1630`): points and the lengths of the segments
/// from each.
const DUST_PATH: [([f32; 3], f32); 9] = [
    ([-50.0, 5.0, 50.0], 100.0),
    ([50.0, 5.0, 50.0], 28.28),
    ([70.0, 5.0, 30.0], 53.85),
    ([70.0, 5.0, -30.0], 5.0),
    ([50.0, 5.0, -50.0], 100.0),
    ([-50.0, 5.0, -50.0], 28.28),
    ([-70.0, 5.0, -30.0], 60.0),
    ([-70.0, 5.0, 30.0], 28.28),
    ([-50.0, 5.0, 50.0], 0.0),
];

/// Five puffs of dust at random places along the path round the stage, `grStadium_801D435C`.
#[doc(alias = "grStadium_801D435C")]
pub fn dust(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    if reg(r.game.grounds[g.0].u.stadium().path_len) == 0.0 {
        for (_, len) in DUST_PATH {
            let v = r.game.grounds[g.0].u.stadium();
            v.path_len = mem(fp::fadds(reg(v.path_len), reg(len)));
        }
    }
    for _ in 0..5 {
        let rand = f64::from(r.game.objs.rng.draw_f32());
        let len = r.game.grounds[g.0].u.stadium().path_len;
        let mut t = fp::fmuls(reg(len), rand);
        let mut k = 0;
        while t > reg(DUST_PATH[k].1) && k < 8 {
            t = fp::fsubs(t, reg(DUST_PATH[k].1));
            k += 1;
        }
        let (a, b) = (DUST_PATH[k].0, DUST_PATH[k + 1].0);
        let mut v = [0f32; 3];
        for i in 0..3 {
            v[i] = mem(fp::fsubs(reg(b[i]), reg(a[i])));
        }
        let f = fp::fdivs(t, reg(DUST_PATH[k].1));
        for x in &mut v {
            *x = mem(fp::fmuls(reg(*x), f));
        }
        for i in 0..3 {
            v[i] = mem(fp::fadds(reg(v[i]), reg(a[i])));
        }
        let s = reg(scale(r.game));
        for x in &mut v {
            *x = mem(fp::fmuls(reg(*x), s));
        }
        grlib::generator_at(r.game, r.data, 0x7548, 0x1E, Vec3::new(v[0], v[1], v[2]));
    }
}

/// Whether Slippi 3.16's online codes (and 3.14's, the same here) freeze the stage
/// (`Online/Core/PreventFileAlarms/FreezeStadium`, `801D4578`): the transformation cycle
/// returns before it does anything, so the stage stays the default one all match and the cycle
/// draws nothing. The set's preload codes (`Common/Preload Stadium Transformations`: picking
/// the next transformation ahead at `801D45EC`, taking it at `801D460C`, `801D4610` and
/// `801D4724`, and clearing their mark at `801D4F14`) all sit in the cycle past that return,
/// so they never run; only their mark's clearing in the setup (`801D14C8`) does.
pub fn frozen(game: &Game) -> bool {
    game.rules.slippi && game.rules.codes != CodeSet::V3_19
}

/// The transformation cycle, `grStadium_801D4548` (the replays' with the Slippi ruleset's codes
/// at `801D457C`, `801D4760` and `801D4FD8`): waiting (0), loading the next stage's file (1),
/// announcing it (2, 3), sinking the current stage (4), raising the next (5), and settling (6).
#[doc(alias = "grStadium_801D4548")]
pub fn cycle(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let s = reg(scale(r.game));
    if frozen(r.game) {
        // Slippi 3.16's `FreezeStadium` (`801D4578`): the cycle returns once it has read the
        // stage's scale.
        return;
    }
    if super::super::gm::is_training(r.game) {
        return;
    }
    if r.game.gr.stkind == 0xF0 {
        if let Some(d) = map_gobj(r.game, MAP_DISPLAY) {
            let dg = ground_of(r.game, d);
            if let Some(b) = r.game.grounds[dg.0].u.stadium_display().subject {
                r.game.camera.boxes[b].state = camera::ACTIVE;
            }
        }
        return;
    }
    let p = param(r.game);
    let d = r.data;
    let state = r.game.grounds[g.0].u.stadium().state;
    match state {
        0 => {
            let v = r.game.grounds[g.0].u.stadium();
            let t = v.timer;
            v.timer = t.wrapping_sub(1);
            if t >= 0 {
                return;
            }
            let kind = if v.kind == MAP_DEFAULT as i16 {
                const KINDS: [i16; 4] = [3, 4, 6, 9];
                loop {
                    let k = KINDS[r.game.objs.rng.below(4) as usize];
                    if r.game.grounds[g.0].u.stadium().prev2 != k {
                        break k;
                    }
                }
            } else {
                MAP_DEFAULT as i16
            };
            let v = r.game.grounds[g.0].u.stadium();
            v.prev2 = v.prev;
            v.prev = v.kind;
            v.kind = kind;
            let idx = match kind {
                5 => {
                    v.state = 2;
                    return;
                }
                3 => 0,
                4 => 1,
                9 => 2,
                6 => 3,
                _ => panic!("the original's assertion failed: grpstadium.c:2628"),
            };
            let slot = v.archive.take();
            free_archive(r.game, slot);
            start_load(r, idx);
            r.game.grounds[g.0].u.stadium().state = 1;
        }
        1 => {
            if load_done(r) {
                r.game.grounds[g.0].u.stadium().state = 2;
            }
        }
        2 => {
            r.game.grounds[g.0].u.stadium().timer = 0;
            if let Some(dgobj) = map_gobj(r.game, MAP_DISPLAY) {
                let mode = match r.game.grounds[g.0].u.stadium().kind {
                    3 => 3,
                    4 => 4,
                    9 => 5,
                    6 => 6,
                    5 => 2,
                    _ => panic!("the original's assertion failed: grpstadium.c:2663"),
                };
                set_mode(r, dgobj, mode, 0);
            }
            r.game.grounds[g.0].u.stadium().state = 3;
        }
        3 => {
            let v = r.game.grounds[g.0].u.stadium();
            let t = v.timer;
            v.timer = t.wrapping_add(1);
            if t > p.i(d, 0x10) {
                let cur = v.current.expect("the current stage");
                let cg = ground_of(r.game, cur);
                r.game.grounds[cg.0].u.stadium_bits().b1 = true;
                let v = r.game.grounds[g.0].u.stadium();
                v.state = 4;
                v.timer = 0;
                r.stage_sound(0x75300);
                r.stage_sound(0x75301);
            }
        }
        4 => {
            let cur = r.game.grounds[g.0]
                .u
                .stadium()
                .current
                .expect("the current stage");
            let j = jobj_of(r.game, cur).expect("HSD_JObjGetScaleY: jobj");
            let sy = reg(r.game.objs.jobjs[j].scale.y);
            let step = fp::fdivs(0.949_999_988_079_071, fp::frsp(f64::from(p.i(d, 0x14))));
            let sy = fp::fnmsubs(s, step, sy);
            let min = fp::fmuls(0.050_000_000_745_058_06, s);
            if sy > min {
                jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.scale.y = mem(sy));
            } else {
                jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.scale.y = 0.05);
                let v = r.game.grounds[g.0].u.stadium();
                let t = v.timer;
                v.timer = t.wrapping_add(1);
                if t > p.i(d, 0x18) {
                    anime::set_rate(r.game, cur, 0, 7, 0.0, true);
                    let kind = i32::from(r.game.grounds[g.0].u.stadium().kind);
                    let new = create(r, kind, NEXT_FRAME).expect("the next stage");
                    let nj = jobj_of(r.game, new).expect("HSD_JObjSetScaleY: jobj");
                    jobj::set_srt(&mut r.game.objs.jobjs, nj, |o| o.scale.y = mem(min));
                    jobj::set_srt(&mut r.game.objs.jobjs, nj, |o| o.translate.y = -10.0);
                    let v = r.game.grounds[g.0].u.stadium();
                    v.next = Some(new);
                    v.timer = 0;
                    v.state = 5;
                    let geo = r.game.geometry_mut();
                    geo.enable_line(0x55);
                    geo.enable_line(0x6F);
                }
            }
            dust(r, gobj);
            camera::request_quake(r, camera::QUAKE_LOOP, None);
        }
        5 => {
            let v = r.game.grounds[g.0].u.stadium();
            v.timer = v.timer.wrapping_add(1);
            let frame = v.timer;
            let (next, cur) = (
                v.next.expect("the next stage"),
                v.current.expect("the stage"),
            );
            let total = p.i(d, 0x14);
            if frame <= total {
                let nj = jobj_of(r.game, next).expect("HSD_JObjSetScaleY: jobj");
                let f = |v: i32| fp::frsp(f64::from(v));
                let tmp = fp::fadds(
                    0.050_000_000_745_058_06,
                    fp::fdivs(fp::fmuls(0.949_999_988_079_071, f(frame)), f(total)),
                );
                jobj::set_srt(&mut r.game.objs.jobjs, nj, |o| {
                    o.scale.y = mem(fp::fmuls(tmp, s))
                });
                let half = total / 2;
                let offset = if frame < half {
                    fp::fmuls(
                        fp::fmuls(fp::fneg(10.0), s),
                        fp::fsubs(1.0, fp::fdivs(f(frame), f(half))),
                    )
                } else {
                    0.0
                };
                jobj::set_srt(&mut r.game.objs.jobjs, nj, |o| o.translate.y = mem(offset));
                let cj = jobj_of(r.game, cur).expect("HSD_JObjSetTranslateY: jobj");
                let half2 = total / 2;
                let offset2 = if frame > half2 {
                    fp::fmuls(
                        fp::fmuls(fp::fneg(10.0), s),
                        fp::fsubs(
                            1.0,
                            fp::fdivs(f(total.wrapping_sub(frame)), f(total.wrapping_sub(half2))),
                        ),
                    )
                } else {
                    0.0
                };
                jobj::set_srt(&mut r.game.objs.jobjs, cj, |o| o.translate.y = mem(offset2));
            } else {
                let nj = jobj_of(r.game, next).expect("HSD_JObjSetScaleY: jobj");
                jobj::set_srt(&mut r.game.objs.jobjs, nj, |o| o.scale.y = mem(s));
                jobj::set_srt(&mut r.game.objs.jobjs, nj, |o| o.translate.y = 0.0);
                r.game.grounds[g.0].u.stadium().state = 6;
                setup::destroy(r, Some(cur));
                let v = r.game.grounds[g.0].u.stadium();
                v.current = Some(next);
                v.next = None;
            }
            dust(r, gobj);
            camera::request_quake(r, camera::QUAKE_LOOP, None);
        }
        6 => {
            let cur = r.game.grounds[g.0].u.stadium().current.expect("the stage");
            let cg = ground_of(r.game, cur);
            r.game.grounds[cg.0].u.stadium_bits().b0 = true;
            r.game.geometry_mut().link_all_joints();
            if r.game.grounds[g.0].u.stadium().kind == MAP_DEFAULT as i16 {
                let t = randi_between(r.game, p.i(d, 0), p.i(d, 4));
                r.game.grounds[g.0].u.stadium().timer = t;
                // `grAnime_801C65B0` on the file's buffer rather than its archive: it zeroes
                // the file's header, which nothing here models, and frees nothing.
                let geo = r.game.geometry_mut();
                geo.disable_line(0x55);
                geo.disable_line(0x6F);
            } else {
                let t = randi_between(r.game, p.i(d, 8), p.i(d, 0xC));
                r.game.grounds[g.0].u.stadium().timer = t;
            }
            r.game.grounds[g.0].u.stadium().state = 0;
        }
        _ => {}
    }
}

/// Frees a stage archive table entry, `grAnime_801C65B0`: an archive the stage loaded itself
/// is freed with it, one it registered from a buffer (`unk8` 1) gives its `HSD_Archive` back
/// to the heap (and its file's material marks, [`setup::mark_materials`], go with it); the
/// entry is cleared. The Slippi ruleset's code there (`801C65C8`) leaves an
/// empty entry alone, where the original would free its null archive.
#[doc(alias = "grAnime_801C65B0")]
pub fn free_archive(game: &mut Game, slot: Option<u8>) {
    let Some(slot) = slot else { return };
    let e = &mut game.gr.archives[slot as usize];
    match e {
        None if game.rules.slippi => {}
        None => panic!("grAnime_801C65B0 frees an empty entry's null archive"),
        Some(a) if a.kind == 0 => {
            panic!("freeing a stage archive the stage loaded itself (lbArchive_80016EFC)")
        }
        Some(a) => {
            // The file's bytes stay in the buffer, its materials' marks with them, until the
            // next transformation's read overwrites them; unregistered, nothing reads them.
            let archive = a.archive;
            crate::hsd::heap::mem_free(a.addr);
            *e = None;
            game.objs
                .rendermode_patches
                .retain(|d, _| d.archive != archive);
        }
    }
}

/// Starts loading transformation file `idx` (the inline `startLoad`): marks the load, then
/// `lbFile_80016580` with `fn_801D4220` to clear the mark when it lands. The Slippi ruleset's
/// code there (`800165AC`) loads it at once instead (`lbFile_8001668C` into the buffer, its
/// archive parsed onto the heap) and registers it in the stage's archive table as the
/// original's wait would (`grDatFiles_801C6478`), leaving the mark; its materials go unmarked,
/// as the code hands `grDatFiles_801C6228` the entry's `map_head` field rather than the
/// `map_head`.
pub fn start_load(r: &mut Running, idx: usize) {
    let g = ground_of(
        r.game,
        map_gobj(r.game, MAP_MAIN).expect("grpstadium.c: map_gobj"),
    );
    r.game.grounds[g.0].u.stadium().bits.b1 = true;
    let name = FILES[idx];
    let archive = r.game.gr.stadium.files[idx]
        .unwrap_or_else(|| panic!("{name} wasn't loaded with the match"));
    // `lbFile_800163D8`, then `lbFile_8001668C`, each write the disc file's length.
    let (entrynum, size) = r.game.ft.data.files.entry(name);
    r.game.grounds[g.0].u.stadium().size = size;
    let buffer = r.game.grounds[g.0].u.stadium().buffer;
    if !r.game.rules.slippi {
        // `lbFile_800164A4`: queued on the device commands, ending in `fn_801D4220`
        // ([`take_load_end`]) on the timeline; only a timeline ends it.
        assert!(
            r.game.audio.clock.is_some(),
            "a transformation's load without the timeline never ends"
        );
        let kind = if buffer >= 0x8000_0000 { 0x21 } else { 0x23 };
        r.game.audio.devcom_request(
            r.data,
            entrynum,
            0,
            buffer,
            size.next_multiple_of(32),
            kind,
            1,
            Some(crate::audio::devcom::cb::STADIUM_LOADED),
            0,
        );
        return;
    }
    r.disc_load(name, buffer);
    let slot = register_archive(r.game, r.data, archive);
    r.game.grounds[g.0].u.stadium().archive = Some(slot);
}

/// The transformation file's load ending, `fn_801D4220`: the device command's callback, which
/// the timeline runs in an interrupt, clears map 2's loading mark. The audio state notes the
/// callback ([`crate::audio::devcom::DevCom::stadium_loaded`]) and the game takes it up before
/// anything reads the mark: after the timeline's stretches around each tick, and when the
/// cycle asks.
#[doc(alias = "fn_801D4220")]
pub fn take_load_end(game: &mut Game) {
    if !std::mem::take(&mut game.audio.devcom.stadium_loaded) {
        return;
    }
    let gm = map_gobj(game, MAP_MAIN).expect("the original's assertion failed: grpstadium.c:2410");
    let g = ground_of(game, gm);
    game.grounds[g.0].u.stadium().bits.b1 = false;
}

/// Registers a loaded transformation file in the stage's archive table's first free entry as
/// loaded from a buffer (`unk8` 1), `grDatFiles_801C6478` without marking its materials: its
/// `HSD_Archive` comes from the scene's heap (`lbHeap_80015BD0`).
fn register_archive(game: &mut Game, data: &Archives, archive: ArchiveId) -> u8 {
    let addr = crate::hsd::heap::mem_alloc(0x44);
    let slot = game
        .gr
        .archives
        .iter()
        .position(Option::is_none)
        .expect("the original's assertion failed: grdatfiles.c:290");
    let map_head = data
        .get(archive)
        .public("map_head")
        .map(|offset| DataRef { archive, offset });
    game.gr.archives[slot] = Some(StageArchive {
        archive,
        map_head,
        kind: 1,
        addr,
    });
    slot as u8
}

/// Whether the transformation file has loaded (the inline `waitLoad`'s `grStadium_801D42B8`).
/// The Slippi ruleset's code there (`801D4760`) asks whether the stage's archive table holds
/// an archive at map 2's map id's entry, whose first word (the file's size) isn't zero.
#[doc(alias = "grStadium_801D42B8")]
fn load_done(r: &mut Running) -> bool {
    let gm = map_gobj(r.game, MAP_MAIN).expect("grpstadium.c: map_gobj");
    if !r.game.rules.slippi {
        // The original's: once `fn_801D4220` cleared the mark, registers the buffer's archive
        // (`grDatFiles_801C6478`, marking its materials).
        take_load_end(r.game);
        let g = ground_of(r.game, gm);
        if r.game.grounds[g.0].u.stadium().bits.b1 {
            return false;
        }
        let idx = match r.game.grounds[g.0].u.stadium().kind {
            3 => 0,
            4 => 1,
            9 => 2,
            6 => 3,
            k => panic!("a transformation file for map {k}"),
        };
        let archive = r.game.gr.stadium.files[idx].expect("the transformation's file");
        let slot = register_archive(r.game, r.data, archive);
        let map_head = r.game.gr.archives[slot as usize]
            .as_ref()
            .and_then(|a| a.map_head);
        setup::mark_materials(r.game, r.data, map_head);
        r.game.grounds[g.0].u.stadium().archive = Some(slot);
        return true;
    }
    let map_id = r.game.grounds[ground_of(r.game, gm).0].map_id;
    // `grDatFiles_801C6330(gp->map_id)`: the entry whose `map_head` has a model for it.
    let slot = r.game.gr.archives.iter().position(|a| {
        a.as_ref().is_some_and(|a| {
            a.map_head.is_some_and(|h| {
                let head = super::datfiles::MapHead(h);
                head.count(r.data) > map_id && head.entry(r.data, map_id).joint(r.data).is_some()
            })
        })
    });
    match slot {
        // The entry's archive's first word: its file's size, never zero for a loaded file.
        Some(s) => r.game.gr.archives[s].is_some(),
        None => panic!("grDatFiles_801C6330 found no archive for map 2 (null->unk0)"),
    }
}

/// Whether a shadow falls on a joint: the point is no more than a unit below the joint's world
/// position, `grStadium_OnCheckShadowRender`.
#[doc(alias = "grStadium_OnCheckShadowRender")]
pub fn check_shadow_render(game: &mut Game, data: &Archives, pos: Vec3, j: Option<JObjId>) -> bool {
    let w = crate::lb::joint::world_position(&mut game.objs, data, j, None);
    fp::fadds(1.0, reg(pos.y)) > reg(w.y)
}

/// The stage's camera subject, if the screen has made it.
pub fn display_subject(game: &mut Game) -> Option<CameraBoxId> {
    let g = map_gobj(game, MAP_DISPLAY)?;
    let gp = ground_of(game, g);
    game.grounds[gp.0].u.stadium_display().subject
}
