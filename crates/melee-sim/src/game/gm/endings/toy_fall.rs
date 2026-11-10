//! The trophy fall (`gmregtyfall.c`, `gm_1A7A.c`, `gmregenddisp.c`): after a single-player
//! mode's clear, the character falls as a demo fighter in pose 7 holding its trophy for the
//! mode on its stand, drawn through copies of the frame (cut 1: the fighter copied each
//! frame and drawn back fading, the whole frame copied every eighth frame and drawn back
//! tinted blue); at the cut's camera's end the fighter is freed and the trophy and stand go
//! on alone (cut 2, `gm_801A7B00`), then stand among every trophy of the mode the save holds,
//! laid round at random (cut 3, `gm_801A9630`), the scene ending 180 ticks after its camera.

use gekko_fp as fp;
use melee_data::archive::{ArchiveId, DataRef};

use super::super::super::handles::CodeRef;
use super::super::super::render::{CAMERA_CALLBACK, FOG_CALLBACK, JOBJ_CALLBACK, LOBJ_CALLBACK};
use super::super::super::state::{OBJ_CAMERA, OBJ_FOG, OBJ_JOBJ, OBJ_LIGHT, Obj, Running};
use super::super::super::vi::Loaded;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sobj::{self, OBJ_SOBJ};
use crate::vec::Vec3;

/// `gm_Scene_ToyFall_OnEnter`, `gm_Scene_ToyFall_OnFrame`.
pub const ON_ENTER: CodeRef = CodeRef(0x801A_7070);
pub const ON_FRAME: CodeRef = CodeRef(0x801A_79D4);

/// The scene's processes and render callbacks.
pub const PROC_MAIN: u32 = 0x801A_6664;
pub const PROC_STAND: u32 = 0x801A_6844;
pub const PROC_FALL: u32 = 0x801A_6868;
pub const RENDER_FRAME: u32 = 0x801A_6A48;
pub const RENDER_FIGHTER: u32 = 0x801A_6ACC;
pub const PROC_CUT1_END: u32 = 0x801A_6B6C;
pub const PROC_CAMERA: u32 = 0x801A_6C30;
pub const RENDER_MODELS: u32 = 0x801A_6D78;
pub const PROC_CUT2_MAIN: u32 = 0x801A_7A44;
pub const PROC_CUT2_STAND: u32 = 0x801A_7A68;
pub const PROC_CUT2_CAMERA: u32 = 0x801A_7A8C;
pub const PROC_CUT3_FOG: u32 = 0x801A_7FB4;
pub const PROC_CUT3_LIGHT: u32 = 0x801A_80CC;
pub const PROC_CUT3_TROPHY: u32 = 0x801A_80F0;
pub const PROC_CUT3_STAND: u32 = 0x801A_851C;
pub const PROC_CUT3_BG: u32 = 0x801A_9498;
pub const PROC_CUT3_CAMERA: u32 = 0x801A_94BC;

/// `gm_803DB2C8`, `gm_803DB2D4`: where the fighter starts and the height it falls to (in
/// 300 steps of the difference).
const START: Vec3 = Vec3 {
    x: 0.0,
    y: 182.0,
    z: 0.0,
};
const LOW: f32 = 125.0;

/// `gm_803DB2EC`: each character's size in the fall (its trophy's and stand's too).
const SCALES: [f32; 33] = [
    0.6, 0.5, 0.75, 0.7, 0.7, 0.4, 0.8, 0.75, 0.9, 0.7, 0.6, 0.6, 0.75, 0.7, 0.65, 0.7, 0.6, 0.7,
    0.8, 0.7, 0.7, 0.7, 1.0, 0.7, 0.4, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
];

/// `gm_804D4278`: the tinted copy's color.
const TINT: [u8; 3] = [0xAA, 0xAA, 0xFF];

/// The game over modes' modes and their own.
const GM_CLASSIC: u8 = 3;
const GM_ADVENTURE: u8 = 4;
const GM_ALLSTAR: u8 = 5;

/// The copies' images: `gm_804808F8.x0[i][j]` at `2i + j` (the fighter's color and depth,
/// two of each), `gm_804809D0[i]` at `4 + i` (the frame), and `gm_80480964.x0[i][j]` at
/// `6 + 2i + j` (allocated and freed, never copied into).
pub const IMAGES: usize = 10;
/// Each copy's width and height (`0x1EA` by `0x1E0`) and where it's taken from (x 60).
pub const COPY_W: u16 = 0x1EA;
pub const COPY_H: u16 = 0x1E0;
pub const COPY_X: u16 = 0x3C;
/// `GX_TF_RGB5A3`, `GX_TF_Z24X8`.
pub const FMT_COLOR: u32 = 5;
pub const FMT_DEPTH: u32 = 0x16;

/// An `HSD_ImageDesc` of `lb_800121FC`: its pixels' block in the scene's heap, size and
/// format.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Frame {
    pub addr: u32,
    pub size: u32,
    pub width: u16,
    pub height: u16,
    pub format: u32,
}

/// The trophy fall's statics (`gmregtyfall.c`'s, `gm_1A7A.c`'s and `gmregenddisp.c`'s).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct ToyFall {
    /// `gm_804D6740`: ticks before Start ends the scene.
    pub wait: u8,
    /// `gm_804D6744`: `GmRegEnd`'s archive.
    pub archive: Loaded,
    /// `gm_804D6748`, `gm_804D67A8`, `gm_804D67A4`, `gm_804D67A0`: the cuts' scenes
    /// (`cut1CanimScene` to `cut3CanimScene`) and cut 3's background (`cut3BgScene`).
    pub scenes: [Option<DataRef>; 4],
    /// `gm_804D67AC`: the stand's scene (`standScene`).
    pub stand_scene: Option<DataRef>,
    /// `gm_804D6798`: the trophy's joint.
    pub trophy: Option<DataRef>,
    /// `gm_804D679C`: the mode's trophy stands (`TyMc*Ds.dat`).
    pub stands: Loaded,
    /// `gm_804D674C`: the fighter.
    pub fighter: Option<GObjId>,
    /// `gm_804D6750`: the fighter's copy's alpha, from 800 down.
    pub fade: i32,
    /// `gm_804D6754`: the fighter's copy's sprite GObj.
    pub copy_gobj: Option<GObjId>,
    /// `gm_804D6758`: the fighter has been copied; `gm_804D675C`: the copy drawn next.
    pub copied: i32,
    pub copy_index: i32,
    /// `gm_804D6760.image`: the image its sprite was last made of (by [`IMAGES`]' index).
    pub copy_image: Option<u8>,
    /// `gm_804D6768`: the trophy's GObj; `gm_804D676C`: a sprite GObj nothing draws on.
    pub main: Option<GObjId>,
    pub spare: Option<GObjId>,
    /// `gm_804D6770`, `gm_804D6774`: cleared, read by nothing.
    pub x6770: i32,
    pub x6774: i32,
    /// `gm_804D6778`: the stand's GObj; `gm_804D677C`: the frame's copy's sprite GObj.
    pub stand: Option<GObjId>,
    pub frame_gobj: Option<GObjId>,
    /// `gm_804D6780`: the frame has been copied; `gm_804D6784`: the copy drawn next.
    pub framed: i32,
    pub frame_index: i32,
    /// `gm_804D6788.image`.
    pub frame_image: Option<u8>,
    /// `gm_804D6790`: ticks until the frame's sprite is made again; `gm_804D6794`: its alpha,
    /// up to 190.
    pub frame_wait: i32,
    pub frame_alpha: i32,
    /// `gm_804808F8`, `gm_804809D0`, `gm_80480964` by [`IMAGES`]' index.
    pub images: [Frame; IMAGES],
    /// `gm_804D67B0`, `gm_804D67B4`: cut 2's trophy and stand.
    pub cut2: [Option<GObjId>; 2],
    /// `gm_80480A00`: cut 3's trophies by character; `gm_80480AD0`: each character's draw.
    pub shown: [Option<GObjId>; 26],
    pub draws: [i32; 26],
    /// `gm_804D67B8`, `gm_804D67BC`, `gm_804D67C0`: cut 3's trophy, stand and background.
    pub cut3: [Option<GObjId>; 3],
    /// `gm_804D67C4`: ticks after cut 3's camera before the scene ends.
    pub end_wait: i32,
}

/// The class and links the scene's GObjs take.
const CLASS_UI: u16 = 0xE;
const CLASS_CAMERA: u16 = 0x13;
const CLASS_LIGHT: u16 = 0xB;

/// `SceneDesc::cameras[0]`: its `desc` and first animation.
fn scene_camera(r: &Running, scene: DataRef) -> (DataRef, Option<DataRef>) {
    let cams = r.data.ptr(scene.at(4)).expect("the scene's cameras");
    let desc = r.data.ptr(cams).expect("the scene's camera");
    let anims = r.data.ptr(cams.at(4));
    (desc, anims.and_then(|a| r.data.ptr(a)))
}

/// `SceneDesc::models[0]` (a `DynamicModelDesc`).
fn scene_model(r: &Running, scene: DataRef) -> DataRef {
    let models = r.data.ptr(scene).expect("the scene's models");
    r.data.ptr(models).expect("the scene's model")
}

impl Running<'_> {
    /// The trophy fall's mode for `gm_80160474`: Classic's, Adventure's, the debug one's
    /// stand-in (`gm_801BF050`), else All-Star's, `gm_801A659C`'s switch.
    fn toy_fall_mode(&self) -> u8 {
        match super::super::super::mn::common::game_mode(self) {
            super::GM_CLASSIC_GOVER => GM_CLASSIC,
            super::GM_ADVENTURE_GOVER => GM_ADVENTURE,
            super::GM_DEBUG_GOVER => self.game.endings.gover.mode(),
            _ => GM_ALLSTAR,
        }
    }

    /// Character `ckind`'s trophy for the mode, `gm_801A659C`.
    #[doc(alias = "gm_801A659C")]
    pub(crate) fn toy_fall_trophy(&self, ckind: u8) -> i32 {
        super::super::unlocks::mode_trophy(ckind, self.toy_fall_mode())
    }

    /// Whether the save holds character `ckind`'s trophy for the mode (`Toy_803048C0`).
    fn toy_fall_held(&self, ckind: u8) -> bool {
        self.game.trophy_count(self.toy_fall_trophy(ckind) as usize) != 0
    }

    /// An image's pixels from the scene's heap (`lb_800121FC` with no preload entry): only
    /// the GPU's copies write them, so the block keeps what it held until the first.
    #[doc(alias = "lb_800121FC")]
    fn toy_fall_image(&mut self, n: usize, format: u32) {
        let size = crate::hsd::heap::tex_buffer_size(u32::from(COPY_W), u32::from(COPY_H), format)
            .next_multiple_of(32);
        let addr = crate::hsd::heap::mem_alloc_unclaimed(size);
        self.game.endings.toy_fall.images[n] = Frame {
            addr,
            size,
            width: COPY_W,
            height: COPY_H,
            format,
        };
    }

    /// `HSD_ImageDescCopyFromEFB(image n, 60, 0, clear, …)`: a draw, and the GPU's pixels in
    /// the image's block, which the simulation never reads.
    fn toy_fall_copy(&mut self, n: u8, clear: bool) {
        self.game
            .objs
            .display
            .draws
            .push(crate::hsd::display::Draw::ToyFallCopy {
                image: n,
                x: COPY_X,
                y: 0,
                clear,
            });
        let f = self.game.endings.toy_fall.images[usize::from(n)];
        if f.addr != 0 {
            crate::hsd::heap::with(|h| {
                h.image.hold(
                    f.addr,
                    f.addr + f.size,
                    crate::hsd::image::Src::RenderTarget,
                )
            });
        }
    }

    /// A camera GObj (class 0x13, p-link 20) with `scene`'s camera (`lb_80013B14`) drawn by
    /// `render` at render priority `prio` over links `prios`, its first animation added,
    /// started and run once, and its process `proc`.
    fn toy_fall_camera(
        &mut self,
        scene: DataRef,
        render: u32,
        prio: u8,
        prios: u64,
        proc: u32,
    ) -> GObjId {
        let (desc, anim) = scene_camera(self, scene);
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_CAMERA, 0x14, 0);
        let c =
            crate::lb::cobj::load_cobj(&mut g.objs, data, crate::lb::cobj::CObjDesc::Data(desc));
        g.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(gobj, CodeRef(render), prio, false);
        g.world.gobj_mut(gobj).gxlink_prios = prios;
        crate::hsd::cobj::add_anim(&mut g.objs, data, Some(c), anim);
        crate::hsd::cobj::req_anim(&mut g.objs, Some(c), 0.0);
        crate::hsd::cobj::anim(&mut g.objs, data, Some(c));
        g.world.setup_proc(gobj, CodeRef(proc), 0);
        gobj
    }

    /// A light GObj (class 0xB, p-link 3) of `scene`'s lights (`lb_80011AC4`).
    fn toy_fall_light(&mut self, scene: DataRef) -> (GObjId, crate::hsd::lobj::LObjId) {
        let lights = self.data.ptr(scene.at(8)).expect("the scene's lights");
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_LIGHT, 3, 0);
        let l = super::super::super::gr::setup::load_light_list(g, data, lights).expect("a light");
        g.world.set_obj(gobj, OBJ_LIGHT, Obj::LObj(l));
        g.world.setup_gx_link(gobj, CodeRef(LOBJ_CALLBACK), 0, 0);
        (gobj, l)
    }

    /// A model GObj (class 0xE, p-link 15) of `joint` drawn on render link 0xB.
    fn toy_fall_model(&mut self, joint: Option<DataRef>) -> (GObjId, JObjId) {
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_UI, 0xF, 0);
        let j = jobj::load_joint(&mut g.objs, data, joint).expect("a model");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 0xB, 0);
        (gobj, j)
    }

    /// A sprite GObj (class 0xE, p-link `plink`) drawn on render link `link`
    /// (`HSD_SObjLib_803A49E0`), with no sprite yet.
    fn toy_fall_sprites(&mut self, plink: u8, link: u8) -> GObjId {
        let w = &mut self.game.world;
        let gobj = w.create(CLASS_UI, plink, 0);
        w.set_obj(gobj, OBJ_SOBJ, Obj::SObj(None));
        w.setup_gx_link(gobj, CodeRef(sobj::RENDER), link, 0);
        gobj
    }

    /// A sprite of image `n` on `gobj` at (60, 0), its color `rgb` (none to keep white) and
    /// alpha `alpha`, its GObj's sprites freed first (`HSD_SObjLib_803A4740`, then
    /// `HSD_SObjLib_803A477C` with the descriptor pointed at the image). The freed sprite's
    /// slot is taken again, which the GObj still names, so the new sprite stands alone.
    fn toy_fall_sprite(&mut self, gobj: GObjId, n: u8, rgb: Option<[u8; 3]>, alpha: u8) {
        let head = match self.game.world.gobj(gobj).obj {
            Some(Obj::SObj(h)) => h,
            ref other => panic!("the trophy fall's sprite GObj holds {other:?}"),
        };
        sobj::free_all(&mut self.game.objs.sobjs, head);
        let f = self.game.endings.toy_fall.images[usize::from(n)];
        let (s, head) = sobj::create(
            &mut self.game.objs.sobjs,
            self.data,
            gobj,
            None,
            sobj::Image::Copy {
                image: n,
                addr: f.addr,
                width: f.width,
                height: f.height,
                format: f.format,
            },
            (0, 0),
            0x80,
            None,
        );
        self.set_sobj_head(gobj, head);
        let o = &mut self.game.objs.sobjs[s.0];
        o.x = 60.0;
        o.y = 0.0;
        if let Some([r, g, b]) = rgb {
            o.color1[0] = r;
            o.color1[1] = g;
            o.color1[2] = b;
        }
        o.color1[3] = alpha;
    }

    /// The trophy's process, `fn_801A6664`: its animation; once the fighter has been copied,
    /// a sprite of the last copy at its alpha (800 down by 6 a tick, by 3 below 256, drawn at
    /// most 255); once the frame has been copied, every eighth tick a sprite of its last copy,
    /// tinted blue, its alpha up by 10 to 190.
    #[doc(alias = "fn_801A6664")]
    pub fn toy_fall_main_proc(&mut self, gobj: GObjId) {
        self.toy_fall_anim(gobj);
        let t = self.game.endings.toy_fall;
        if t.copied != 0 {
            let n = (2 * t.copy_index) as u8;
            self.game.endings.toy_fall.copy_image = Some(n);
            let t = &mut self.game.endings.toy_fall;
            t.copy_index = i32::from(t.copy_index == 0);
            if t.fade != 0 {
                t.fade -= if t.fade > 0xFF { 6 } else { 3 };
                if t.fade < 0 {
                    t.fade = 0;
                }
            }
            let alpha = t.fade.min(0xFF) as u8;
            let g = t.copy_gobj.expect("the fighter's copy's sprite GObj");
            self.toy_fall_sprite(g, n, None, alpha);
        }
        let t = self.game.endings.toy_fall;
        if t.framed != 0 {
            if t.frame_wait != 0 {
                self.game.endings.toy_fall.frame_wait -= 1;
                return;
            }
            let t = &mut self.game.endings.toy_fall;
            t.frame_wait = 7;
            let n = (4 + t.frame_index) as u8;
            t.frame_image = Some(n);
            if t.frame_alpha < 0xBE {
                t.frame_alpha = (t.frame_alpha + 0xA).min(0xBE);
            }
            let alpha = t.frame_alpha as u8;
            let g = t.frame_gobj.expect("the frame's copy's sprite GObj");
            t.frame_index = i32::from(t.frame_index == 0);
            self.toy_fall_sprite(g, n, Some(TINT), alpha);
        }
    }

    /// A model GObj's animations (`fn_801A6844`, `fn_801A7A44`, `fn_801A7A68`,
    /// `fn_801A80F0`, `fn_801A9498`).
    #[doc(alias = "fn_801A6844")]
    #[doc(alias = "fn_801A7A44")]
    #[doc(alias = "fn_801A7A68")]
    #[doc(alias = "fn_801A80F0")]
    #[doc(alias = "fn_801A9498")]
    pub fn toy_fall_anim(&mut self, gobj: GObjId) {
        let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj else {
            panic!("a trophy fall model holds no JObj");
        };
        self.anim_all(j);
    }

    /// The fighter's fall, `fn_801A6868`: from its height down to 125, by a 300th of the
    /// distance from the start a tick.
    #[doc(alias = "fn_801A6868")]
    pub fn toy_fall_fall(&mut self) {
        let mut p = self.game.player_pos(0);
        if p.y >= LOW {
            let step = fp::fdivs(fp::fsubs(reg(LOW), reg(START.y)), 300.0);
            p.y = mem(fp::fadds(reg(p.y), step));
            self.player_set_position(0, p);
        }
    }

    /// Cut 1's fighter and its stage, `gm_801A68D8`: the dummy stage's scene with the common
    /// items set up before the stage, the effects, the demo fighters' pools, the character's
    /// ending animations from `GmRegEnd`, its slot as a demo player at the start facing
    /// right, its fighters in pose 7 (falling, `ftCo_800C6150`) with their animations at 0.6
    /// their rate, the fall, and its size the character's over its own.
    #[doc(alias = "gm_801A68D8")]
    // `gm_GetCurrentSceneEnterData`: read here and by the scene's other setups, its answer
    // discarded.
    #[doc(alias = "gm_GetCurrentSceneEnterData")]
    fn toy_fall_fighter(&mut self) {
        super::super::super::camera::init(&mut self.game.camera, 6);
        self.game.alloc_dynamics(self.data);
        self.game.coll.reset();
        super::super::super::gr::setup::prepare(self.game);
        super::super::super::gr::setup::select(self, 0, None);
        let kept = self.take_cached_named("ItCo.usd");
        let it_co = self.game.ft.data.files.archive("ItCo.usd");
        super::super::super::it::item::load_common_data(self.game, self.data, it_co, kept);
        super::super::super::it::item::init_globals(self.game, self.data);
        let grkind = self.game.stage_info.grkind;
        super::super::super::gr::setup::setup(self, grkind);
        super::super::super::ef::eflib::init(self);
        super::super::super::ef::load_sync(self, 0);
        self.fighter_demo_init();
        let ckind = self.game.endings.gover.ckind();
        let a = self.game.endings.toy_fall.archive;
        self.player_set_demo_archive_idx(
            i32::from(ckind),
            a.archive.expect("GmRegEnd's archive"),
            a.file,
            2,
        );
        self.game.reinit_player(0);
        let costume = self.game.endings.gover.color();
        self.game.set_player_ckind(0, i32::from(ckind));
        self.game.set_player_costume(0, i32::from(costume));
        self.game.set_player_port(0, 0);
        self.game
            .set_player_pkind(0, super::super::super::pl::player::PKIND_DEMO);
        self.game.set_player_facing(0, 1.0);
        self.game.set_player_pos(0, START);
        self.player_create_demo(0, 7);
        let f = self.game.player_entity(0).expect("the falling fighter");
        self.game.endings.toy_fall.fighter = Some(f);
        let root = self.root_jobj(f);
        crate::lb::anim::set_framerate(&mut self.game.objs, Some(root), 0.6);
        self.game.world.setup_proc(f, CodeRef(PROC_FALL), 4);
        if let Some(f2) = self.game.player_entity_at(0, 1) {
            let root = self.root_jobj(f2);
            crate::lb::anim::set_framerate(&mut self.game.objs, Some(root), 0.6);
        }
        let size = reg(self.player_scale_ratio(0));
        let inv = fp::fdivs(1.0, size);
        let s = mem(fp::fmuls(inv, reg(SCALES[usize::from(ckind)])));
        self.player_set_scale(0, s);
    }

    /// The fighter's camera's render callback, `fn_801A6ACC`: the fighters' passes, then the
    /// frame copied into the next copy's color image and its depth image, cleared after.
    #[doc(alias = "fn_801A6ACC")]
    pub fn toy_fall_fighter_render(&mut self, gobj: GObjId) {
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("the trophy fall's camera holds no CObj");
        };
        if crate::hsd::display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
            self.camera_fighters(gobj, 1);
            let t = &mut self.game.endings.toy_fall;
            t.copied = 1;
            let n = (2 * t.copy_index) as u8;
            self.toy_fall_copy(n, false);
            self.toy_fall_copy(n + 1, true);
            crate::hsd::display::camera_end(&mut self.game.objs);
        }
    }

    /// The frame's sprite camera's render callback, `fn_801A6A48`: its sprites, then the
    /// frame copied into the next frame image.
    #[doc(alias = "fn_801A6A48")]
    pub fn toy_fall_frame_render(&mut self, gobj: GObjId) {
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("the trophy fall's sprite camera holds no CObj");
        };
        if crate::hsd::display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
            self.render_sprite_camera(gobj);
            let t = &mut self.game.endings.toy_fall;
            t.framed = 1;
            let n = (4 + t.frame_index) as u8;
            self.toy_fall_copy(n, false);
            crate::hsd::display::camera_end(&mut self.game.objs);
        }
    }

    /// The models' camera's render callback, `fn_801A6D78`: passes 0 to 2 of its links.
    #[doc(alias = "fn_801A6D78")]
    pub fn toy_fall_models_render(&mut self, gobj: GObjId) {
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("the trophy fall's camera holds no CObj");
        };
        if crate::hsd::display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
            self.render_links(gobj, 7);
            crate::hsd::display::camera_end(&mut self.game.objs);
        }
    }

    /// A camera's animation (`fn_801A6C30`).
    #[doc(alias = "fn_801A6C30")]
    pub fn toy_fall_camera_proc(&mut self, gobj: GObjId) {
        let c = self.toy_fall_cobj(gobj);
        crate::hsd::cobj::anim(&mut self.game.objs, self.data, Some(c));
    }

    fn toy_fall_cobj(&self, gobj: GObjId) -> crate::hsd::cobj::CObjId {
        match self.game.world.gobj(gobj).obj {
            Some(Obj::CObj(c)) => c,
            ref other => panic!("a trophy fall camera holds {other:?}"),
        }
    }

    /// Whether a camera's animation is at its last frame.
    fn toy_fall_camera_done(&self, c: crate::hsd::cobj::CObjId) -> (f32, f32) {
        let objs = &self.game.objs;
        let a = objs.cobjs[c].aobj.expect("a camera without an animation");
        (objs.aobjs[a].curr_frame, objs.aobjs[a].end_frame)
    }

    /// The models' camera's process, `fn_801A6B6C`: its animation; at its end the fighters
    /// freed (`Player_80031EBC`), the trophy, the stand and the sprite GObjs, the light, and
    /// the spare copies' pixels; then cut 2, and the camera's own GObj.
    #[doc(alias = "fn_801A6B6C")]
    pub fn toy_fall_cut1_end(&mut self, gobj: GObjId) {
        let c = self.toy_fall_cobj(gobj);
        crate::hsd::cobj::anim(&mut self.game.objs, self.data, Some(c));
        let (cur, end) = self.toy_fall_camera_done(c);
        if cur != end {
            return;
        }
        self.player_free_fighters(0);
        let t = self.game.endings.toy_fall;
        for g in [t.main, t.stand, t.copy_gobj, t.spare, t.frame_gobj] {
            crate::hsd::gobj::free(self, g.expect("a trophy fall GObj"));
        }
        self.free_gobj_links(3, 3);
        for n in 6..IMAGES {
            crate::hsd::heap::mem_free(self.game.endings.toy_fall.images[n].addr);
        }
        self.toy_fall_cut2();
        crate::hsd::gobj::free(self, gobj);
    }

    /// The fighter's camera and its copies' images, `gm_801A6C54`: cut 1's camera drawn by
    /// `fn_801A6ACC` at render priority 8 over links 0, 5 and 6, animated, and two color and
    /// two depth images.
    #[doc(alias = "gm_801A6C54")]
    fn toy_fall_fighter_camera(&mut self) {
        let scene = self.game.endings.toy_fall.scenes[0].expect("cut 1's scene");
        self.toy_fall_camera(scene, RENDER_FIGHTER, 8, 0x61, PROC_CAMERA);
        for i in 0..2 {
            self.toy_fall_image(2 * i, FMT_COLOR);
            self.toy_fall_image(2 * i + 1, FMT_DEPTH);
        }
    }

    /// The models' camera, `gm_801A6DC0`: cut 1's camera again drawn by `fn_801A6D78` at
    /// render priority 11 over links 0 and 11, animated, ending the cut, and four more images
    /// nothing copies into.
    #[doc(alias = "gm_801A6DC0")]
    fn toy_fall_models_camera(&mut self) {
        let scene = self.game.endings.toy_fall.scenes[0].expect("cut 1's scene");
        self.toy_fall_camera(scene, RENDER_MODELS, 0xB, 0x801, PROC_CUT1_END);
        for i in 0..2 {
            self.toy_fall_image(6 + 2 * i, FMT_COLOR);
            self.toy_fall_image(6 + 2 * i + 1, FMT_DEPTH);
        }
    }

    /// The scene's files, `gm_801A6EE4`: the region's class 2 trophies marked, the trophy
    /// data, the trophy (its joint), `GmRegEnd` (the cuts' scenes), the stand and the mode's
    /// trophy stands.
    #[doc(alias = "gm_801A6EE4")]
    #[doc(alias = "gm_801604DC")]
    #[doc(alias = "gm_80160564")]
    fn toy_fall_files(&mut self) {
        self.mark_class_two_trophies();
        self.toy_load_dataf();
        let ckind = self.game.endings.gover.ckind();
        let mode = self.toy_fall_mode();
        let file = self.toy_model_file(super::super::unlocks::mode_trophy(ckind, mode));
        let (name, sym) = (self.toy_file_name(file), self.toy_file_symbol(file));
        let (ta, _, _) = self.load_archive_blocks(&name);
        let trophy = self.toy_fall_symbol(ta, &sym);
        let name = toy_fall_archive_name(self.files_us());
        let (a, file, block) = self.load_archive_blocks(name);
        let scenes = [
            "cut1CanimScene",
            "cut2CanimScene",
            "cut3CanimScene",
            "cut3BgScene",
        ]
        .map(|s| Some(self.toy_fall_symbol(a, s)));
        let (sa, _, _) = self.load_archive_blocks("GmRgStnd.dat");
        let stand = self.toy_fall_symbol(sa, "standScene");
        let name = stands_file(mode);
        let (da, dfile, dblock) = self.load_archive_blocks(name);
        let t = &mut self.game.endings.toy_fall;
        t.trophy = Some(trophy);
        t.archive = Loaded {
            archive: Some(a),
            file,
            block,
        };
        t.scenes = scenes;
        t.stand_scene = Some(stand);
        t.stands = Loaded {
            archive: Some(da),
            file: dfile,
            block: dblock,
        };
    }

    fn toy_fall_symbol(&self, a: ArchiveId, sym: &str) -> DataRef {
        let offset = self
            .data
            .get(a)
            .public(sym)
            .unwrap_or_else(|| panic!("{sym} isn't in its archive"));
        DataRef { archive: a, offset }
    }

    /// `gm_Scene_ToyFall_OnEnter`: the statics cleared, the scene's sound banks, its files,
    /// cut 1's light and two cameras, two sprite cameras over the screen (the frame's at
    /// render priority 12 over link 18, copying the frame after; the plain one at 9 over
    /// link 9), the frame images, three sprite GObjs, the fighter, the trophy at its hip and
    /// the stand on the trophy, and the fall's sound.
    #[doc(alias = "gm_Scene_ToyFall_OnEnter")]
    #[doc(alias = "HSD_SObjLib_803A55DC")]
    pub fn toy_fall_on_enter(&mut self) {
        // The scene takes the files the preload cache holds (`lbArchive_80017040`): the
        // common effects and items.
        self.game.ft.data.files.from_cache = true;
        let t = &mut self.game.endings.toy_fall;
        t.wait = 0x1E;
        t.fade = 0x320;
        t.copy_gobj = None;
        t.copied = 0;
        t.copy_index = 0;
        t.spare = None;
        t.x6770 = 0;
        t.x6774 = 0;
        t.frame_gobj = None;
        t.framed = 0;
        t.frame_index = 0;
        t.frame_alpha = 0;
        t.frame_wait = 7;
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x1C);
        a.lb.want_banks(0x1C, 0x0010_0000_0000_0000);
        a.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
        self.toy_fall_files();
        let scene = self.game.endings.toy_fall.scenes[0].expect("cut 1's scene");
        self.toy_fall_light(scene);
        self.toy_fall_fighter_camera();
        self.toy_fall_models_camera();
        let g = self.game.world.create(CLASS_CAMERA, 0x14, 0);
        self.sprite_camera(g, 0x280, 0x1E0, 0xC, false);
        self.game.world.gx_unlink(g);
        self.game
            .world
            .setup_gx_link_max(g, CodeRef(RENDER_FRAME), 0xC, false);
        self.game.world.gobj_mut(g).gxlink_prios = 0x4_0000;
        let g = self.game.world.create(CLASS_CAMERA, 0x14, 0);
        self.sprite_camera(g, 0x280, 0x1E0, 9, false);
        self.game.world.gx_unlink(g);
        self.game
            .world
            .setup_gx_link_max(g, CodeRef(sobj::CAMERA_RENDER), 9, false);
        self.game.world.gobj_mut(g).gxlink_prios = 0x200;
        for i in 0..2 {
            self.toy_fall_image(4 + i, FMT_COLOR);
        }
        let g = self.toy_fall_sprites(0xD, 9);
        self.game.endings.toy_fall.frame_gobj = Some(g);
        let g = self.toy_fall_sprites(0x18, 0x12);
        self.game.endings.toy_fall.spare = Some(g);
        let g = self.toy_fall_sprites(0x18, 0x12);
        self.game.endings.toy_fall.copy_gobj = Some(g);
        self.toy_fall_fighter();
        self.toy_fall_main();
        self.toy_fall_stand();
        self.sfx_play(0x7EF40, 0x7F, 0x40);
    }

    /// The trophy at the character's size following the fighter's hip
    /// (`gm_801A7070_SetupMain`, inline in the entry).
    fn toy_fall_main(&mut self) {
        let joint = self.game.endings.toy_fall.trophy;
        let (gobj, j) = self.toy_fall_model(joint);
        self.game.endings.toy_fall.main = Some(gobj);
        let s = SCALES[usize::from(self.game.endings.gover.ckind())];
        set_scale_each(&mut self.game.objs.jobjs, j, s);
        self.game.world.setup_proc(gobj, CodeRef(PROC_MAIN), 0x17);
        let f = self.game.player_entity(0).expect("the falling fighter");
        let hip = self.part_joint(f, super::super::super::ft::parts::HIP_N);
        let target = descend(&self.game.objs.jobjs, j, 4).expect("the trophy's fourth joint down");
        let hip = hip.expect("the fighter's hip");
        super::super::super::ef::constrain(&mut self.game.objs, target, hip, 0x9000_0001);
        super::super::super::ef::constrain(&mut self.game.objs, target, hip, 0x9000_0004);
    }

    /// The stand placed for the trophy (`Toy_803060BC`'s fields, negated but the scale, the
    /// trophy's stand size over its size) at the character's size, following the trophy's
    /// fifth joint down's next (`gm_801A7070_SetupTrophy`, inline in the entry).
    fn toy_fall_stand(&mut self) {
        let stand = self
            .game
            .endings
            .toy_fall
            .stand_scene
            .expect("the stand's scene");
        let joint = self.data.ptr(scene_model(self, stand));
        let (gobj, j) = self.toy_fall_model(joint);
        self.game.endings.toy_fall.stand = Some(gobj);
        let trophy = self.toy_fall_trophy(self.game.endings.gover.ckind());
        self.toy_fall_place_stand(j, trophy);
        let s = SCALES[usize::from(self.game.endings.gover.ckind())];
        set_scale_each(&mut self.game.objs.jobjs, j, s);
        self.game.world.setup_proc(gobj, CodeRef(PROC_STAND), 0x17);
        let main = self.game.endings.toy_fall.main.expect("the trophy");
        let Some(Obj::JObj(mj)) = self.game.world.gobj(main).obj else {
            panic!("the trophy holds no JObj");
        };
        let target =
            descend(&self.game.objs.jobjs, mj, 5).and_then(|n| self.game.objs.jobjs[n].next);
        self.toy_fall_follow(j, target);
    }

    /// `j` follows `target`'s place and orientation (`lb_8000C1C0`, `lb_8000C290`).
    fn toy_fall_follow(&mut self, j: JObjId, target: Option<JObjId>) {
        let target = target.expect("a trophy fall constraint joint");
        super::super::super::ef::constrain(&mut self.game.objs, j, target, 0x9000_0001);
        super::super::super::ef::constrain(&mut self.game.objs, j, target, 0x9000_0004);
    }

    /// The stand's child placed for `trophy`: the trophy's offset and turn negated, its scale
    /// the trophy's stand size over its size (the `WithMtxDirty` setters).
    fn toy_fall_place_stand(&mut self, j: JObjId, trophy: i32) {
        let child = self.game.objs.jobjs[j].child.expect("the stand's child");
        let p = |r: &Self, f: u32| r.toy_param(trophy, f);
        let (x, y, z) = (p(self, 0), p(self, 1), p(self, 2));
        let turn = mem(fp::fneg(fp::fmuls(reg(0.017_453_292), reg(p(self, 5)))));
        let inv = fp::fdivs(1.0, reg(p(self, 3)));
        let s = mem(fp::fmuls(reg(p(self, 4)), inv));
        let jobjs = &mut self.game.objs.jobjs;
        jobj::set_srt(jobjs, child, |o| o.translate.x = mem(fp::fneg(reg(x))));
        jobj::set_srt(jobjs, child, |o| o.translate.y = mem(fp::fneg(reg(y))));
        jobj::set_srt(jobjs, child, |o| o.translate.z = mem(fp::fneg(reg(z))));
        jobj::set_rotation_y(jobjs, child, turn);
        set_scale_each(jobjs, child, s);
    }

    /// `gm_Scene_ToyFall_OnFrame`: after 30 ticks, Start on the human's port ends the scene.
    #[doc(alias = "gm_Scene_ToyFall_OnFrame")]
    pub fn toy_fall_on_frame(&mut self) {
        let t = &mut self.game.endings.toy_fall;
        if t.wait != 0 {
            t.wait -= 1;
            return;
        }
        let port = self.game.endings.gover.port();
        if self.game.buttons_triggered(port) & u64::from(crate::input::pad::BUTTON_START) != 0 {
            self.sfx_stop_all();
            self.lb_audio_call(1);
            self.game.end_scene();
        }
    }

    /// Cut 2, `gm_801A7B00`: its light, its camera (render priority 8 over links 0 and 11,
    /// ending the cut), the trophy with cut 2's model's animations, and the stand on it.
    #[doc(alias = "gm_801A7B00")]
    fn toy_fall_cut2(&mut self) {
        let scene = self.game.endings.toy_fall.scenes[1].expect("cut 2's scene");
        self.toy_fall_light(scene);
        self.toy_fall_camera(scene, CAMERA_CALLBACK, 8, 0x801, PROC_CUT2_CAMERA);
        let joint = self.game.endings.toy_fall.trophy;
        let (gobj, j) = self.toy_fall_model(joint);
        self.game.endings.toy_fall.cut2[0] = Some(gobj);
        let m = scene_model(self, scene);
        super::super::vs::add_model_anim(&mut self.game.objs, self.data, Some(j), m, 0);
        jobj::req_anim_all(&mut self.game.objs, Some(j), 0.0);
        self.anim_all(j);
        self.game
            .world
            .setup_proc(gobj, CodeRef(PROC_CUT2_MAIN), 0x17);
        let stand = self
            .game
            .endings
            .toy_fall
            .stand_scene
            .expect("the stand's scene");
        let joint = self.data.ptr(scene_model(self, stand));
        let (sg, sj) = self.toy_fall_model(joint);
        self.game.endings.toy_fall.cut2[1] = Some(sg);
        let trophy = self.toy_fall_trophy(self.game.endings.gover.ckind());
        self.toy_fall_place_stand(sj, trophy);
        self.game
            .world
            .setup_proc(sg, CodeRef(PROC_CUT2_STAND), 0x17);
        let target =
            descend(&self.game.objs.jobjs, j, 5).and_then(|n| self.game.objs.jobjs[n].next);
        self.toy_fall_follow(sj, target);
    }

    /// Cut 2's camera's process, `fn_801A7A8C`: its animation; at its end the trophy, the
    /// stand and the light freed, cut 3, and the camera's own GObj.
    #[doc(alias = "fn_801A7A8C")]
    pub fn toy_fall_cut2_camera(&mut self, gobj: GObjId) {
        let c = self.toy_fall_cobj(gobj);
        crate::hsd::cobj::anim(&mut self.game.objs, self.data, Some(c));
        let (cur, end) = self.toy_fall_camera_done(c);
        if cur != end {
            return;
        }
        let [a, b] = self.game.endings.toy_fall.cut2;
        crate::hsd::gobj::free(self, a.expect("cut 2's trophy"));
        crate::hsd::gobj::free(self, b.expect("cut 2's stand"));
        self.free_gobj_links(3, 3);
        self.toy_fall_cut3();
        crate::hsd::gobj::free(self, gobj);
    }

    /// How many characters' trophies for the mode the save holds (`fn_801A7FB4_inline`).
    fn toy_fall_held_count(&self) -> i32 {
        (0..0x1A).filter(|&i| self.toy_fall_held(i)).count() as i32
    }

    /// Cut 3's fog's process, `fn_801A7FB4`: its animation, held at frame 185 with five
    /// trophies held or fewer, at 200 with thirteen or fewer.
    #[doc(alias = "fn_801A7FB4")]
    pub fn toy_fall_cut3_fog(&mut self, gobj: GObjId) {
        let Some(Obj::Fog(f)) = self.game.world.gobj(gobj).obj else {
            panic!("cut 3's fog GObj holds no fog");
        };
        let count = self.toy_fall_held_count();
        let a = self.game.objs.fogs[f].aobj.expect("the fog's animation");
        let frame = self.game.objs.aobjs[a].curr_frame;
        let run = if count <= 5 {
            frame < 185.0
        } else if count <= 0xD {
            frame < 200.0
        } else {
            true
        };
        if run {
            crate::hsd::fog::interpret_anim(&mut self.game.objs, self.data, Some(f));
        }
    }

    /// Cut 3's light's process, `fn_801A80CC`: its animations.
    #[doc(alias = "fn_801A80CC")]
    pub fn toy_fall_cut3_light(&mut self, gobj: GObjId) {
        let Some(Obj::LObj(l)) = self.game.world.gobj(gobj).obj else {
            panic!("cut 3's light GObj holds no light");
        };
        crate::hsd::lobj::anim_all(&mut self.game.objs, self.data, Some(l));
    }

    /// Cut 3's stand's process, `fn_801A851C`: its animations, its depth 2, and at the
    /// scene's tick 372 a sound.
    #[doc(alias = "fn_801A851C")]
    pub fn toy_fall_cut3_stand(&mut self, gobj: GObjId) {
        let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj else {
            panic!("cut 3's stand holds no JObj");
        };
        self.anim_all(j);
        jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.translate.z = 2.0);
        if self.game.scene_ticks() == 0x174 {
            self.sfx_play(0x7EF41, 0x7F, 0x40);
        }
    }

    /// Cut 3's camera's process, `fn_801A94BC`: its animation to frame 160 with five trophies
    /// held or fewer, 190 with thirteen or fewer, else its end; then 180 ticks and the
    /// scene's end.
    #[doc(alias = "fn_801A94BC")]
    pub fn toy_fall_cut3_camera(&mut self, gobj: GObjId) {
        let c = self.toy_fall_cobj(gobj);
        let count = self.toy_fall_held_count();
        let (cur, end) = self.toy_fall_camera_done(c);
        let run = if count <= 5 {
            cur < 160.0
        } else if count <= 0xD {
            cur < 190.0
        } else {
            cur < end
        };
        if run {
            crate::hsd::cobj::anim(&mut self.game.objs, self.data, Some(c));
        } else {
            let t = &mut self.game.endings.toy_fall;
            t.end_wait -= 1;
            if t.end_wait == 0 {
                self.game.end_scene();
            }
        }
    }

    /// Cut 3's trophies in their order, `gm_801A8D54`: the characters whose trophy for the
    /// mode the save holds but the character's own (and Zelda's and Sheik's for either),
    /// sorted by their draws (an exchange sort, a pair swapped unless the first draw is the
    /// smaller), 26 for none.
    #[doc(alias = "gm_801A8D54")]
    fn toy_fall_order(&self) -> [i32; 26] {
        let ckind = self.game.endings.gover.ckind();
        let mut list = [0x1A_i32; 26];
        let mut count = 0;
        for i in 0..0x1A_u8 {
            let take = if i.wrapping_sub(0x12) <= 1 {
                self.toy_fall_held(i) && ckind != 0x12 && ckind != 0x13
            } else {
                self.toy_fall_held(i) && i != ckind
            };
            if take {
                list[count] = i32::from(i);
                count += 1;
            }
        }
        let draws = self.game.endings.toy_fall.draws;
        for i in 0..26 {
            if list[i] == 0x1A {
                continue;
            }
            for j in i + 1..26 {
                if list[j] != 0x1A && draws[list[i] as usize] >= draws[list[j] as usize] {
                    list.swap(i, j);
                }
            }
        }
        list
    }

    /// Cut 3's trophies, `gm_801A9094`: from the last, each character's trophy's display
    /// model and material animation from the mode's stands at its frame and place, on a
    /// display stand, under a root at 1.8 the size, laid round (`gm_801A85E4`).
    #[doc(alias = "gm_801A9094")]
    #[doc(alias = "tyDisplay_8031BB34")]
    #[doc(alias = "tyDisplay_8031BB94")]
    fn toy_fall_trophies(&mut self) {
        let order = self.toy_fall_order();
        let stands = self
            .game
            .endings
            .toy_fall
            .stands
            .archive
            .expect("the mode's stands");
        for i in (0..26).rev() {
            let ck = order[i];
            if ck == 0x1A {
                continue;
            }
            let dsp = self.coll_entry(self.toy_fall_trophy(ck as u8));
            let cat = super::super::super::ty::collection::category_index(dsp.category as i8);
            let joint = self
                .data
                .get(stands)
                .public(super::super::super::ty::collection::JOINTS[cat])
                .map(|offset| DataRef {
                    archive: stands,
                    offset,
                });
            let name = super::super::super::ty::collection::matanim(cat);
            let matanim = self.data.get(stands).public(&name).map(|offset| DataRef {
                archive: stands,
                offset,
            });
            let bg = self
                .data
                .get(stands)
                .public("ToyDspStand_Top_joint")
                .map(|offset| DataRef {
                    archive: stands,
                    offset,
                });
            let data = self.data;
            let g = &mut *self.game;
            let gobj = g.world.create(CLASS_UI, 0xF, 0);
            g.endings.toy_fall.shown[ck as usize] = Some(gobj);
            let root = jobj::alloc(&mut g.objs);
            g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(root));
            g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 0xB, 0);
            set_scale_each(&mut g.objs.jobjs, root, 1.8);
            assert!(joint.is_some(), "the original's assertion failed: joint");
            let j = jobj::load_joint(&mut g.objs, data, joint).expect("a display model");
            jobj::add_child(&mut g.objs.jobjs, Some(root), Some(j));
            jobj::add_anim_all(&mut g.objs, data, Some(j), None, matanim, None);
            jobj::req_anim_all(&mut g.objs, Some(j), fp::frsp(f64::from(dsp.frame)) as f32);
            self.anim_all(j);
            let jobjs = &mut self.game.objs.jobjs;
            jobj::set_srt(jobjs, j, |o| o.translate.x = dsp.x);
            jobj::set_srt(jobjs, j, |o| o.translate.z = dsp.z);
            let b = jobj::load_joint(&mut self.game.objs, data, bg);
            jobj::add_child(&mut self.game.objs.jobjs, Some(root), b);
            self.toy_fall_lay(root, i as i32);
        }
    }

    /// A trophy's place among cut 3's, `gm_801A85E4`, by its place `n` in the order: the
    /// first five on a ring of 25 (the angle 45 degrees a step, up to 2 off at random), the
    /// next eight on one of 50, the next nine on one of 75, the last three at fixed places;
    /// each ring 18 back and pressed to 0.9 (0.8 the last) in depth.
    #[doc(alias = "gm_801A85E4")]
    fn toy_fall_lay(&mut self, j: JObjId, n: i32) {
        let deg = reg(0.017_453_292);
        let set = |r: &mut Self, x: f64, z: f64| {
            let jobjs = &mut r.game.objs.jobjs;
            jobj::set_srt(jobjs, j, |o| o.translate.x = mem(x));
            jobj::set_srt(jobjs, j, |o| o.translate.z = mem(z));
        };
        // Each ring: its first place and last, the angle's step and its first index's offset,
        // the random spread and its half, the radius and the depth's squeeze.
        let rings: [Ring; 3] = [
            (2, 4, 45.0, 0, 2.0, 1.0, 25.0, 0.9),
            (8, 12, 25.714_285, 5, 4.0, 2.0, 50.0, 0.9),
            (0x11, 0x15, 22.5, 0xD, 1.0, 0.5, 75.0, 0.8),
        ];
        let tier = if n <= 4 {
            Some(0)
        } else if n <= 0xC {
            Some(1)
        } else if n <= 0x15 {
            Some(2)
        } else {
            None
        };
        match tier {
            Some(t) => {
                let (base, last, step, off, spread, half, radius, squeeze) = rings[t];
                let mut idx = last - n + base;
                if idx > last {
                    idx = last - idx + base;
                }
                let rand = reg(self.game.objs.rng.draw_f32());
                let rand = if spread == 1.0 {
                    rand
                } else {
                    fp::fmuls(spread, rand)
                };
                let a = fp::fmadds(reg(step), fp::frsp(f64::from(idx - off)), rand);
                let angle = fp::fmuls(deg, fp::fsubs(a, half));
                let c = reg(crate::math::cos(mem(angle)));
                let s = reg(crate::math::sin(mem(angle)));
                let x = fp::fmuls(radius, c);
                let z = fp::fmuls(reg(squeeze), fp::fmadds(radius, fp::fneg(s), -18.0));
                set(self, x, z);
            }
            None => {
                let (x, z) = match n {
                    0x16 => (-80.0, -70.0),
                    0x17 => (75.0, -75.0),
                    _ => (90.0, -55.0),
                };
                set(self, x, z);
            }
        }
    }

    /// Cut 3, `gm_801A9630`: each character's draw (`HSD_Randi(10000)` for a trophy held, else
    /// 0) and 180 ticks' wait; the fog, the light and the camera of cut 3's scene, animated;
    /// the background; the stand with cut 3's model's animations; the trophies; and the
    /// trophy, placed on its stand as the trophy screens do (`gm_801A8114`) and following the
    /// stand's third joint down.
    #[doc(alias = "gm_801A9630")]
    #[doc(alias = "gm_801A8114")]
    fn toy_fall_cut3(&mut self) {
        let t = &mut self.game.endings.toy_fall;
        t.shown = [None; 26];
        t.end_wait = 0xB4;
        for i in 0..0x1A_u8 {
            let d = if self.toy_fall_held(i) {
                self.game.objs.rng.below(0x2710)
            } else {
                0
            };
            self.game.endings.toy_fall.draws[usize::from(i)] = d;
        }
        let scene = self.game.endings.toy_fall.scenes[2].expect("cut 3's scene");
        // The fog.
        let data = self.data;
        let fogs = data.ptr(scene.at(0xC)).expect("the scene's fogs");
        let desc = data.ptr(fogs).expect("the scene's fog");
        let anims = data.ptr(fogs.at(4)).expect("the fog's animations");
        let anim = data.ptr(anims).and_then(|a| data.ptr(a));
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_UI, 3, 0);
        let f = crate::hsd::fog::load_desc(&mut g.objs.fogs, data, desc);
        g.world.set_obj(gobj, OBJ_FOG, Obj::Fog(f));
        g.world.setup_gx_link(gobj, CodeRef(FOG_CALLBACK), 0, 0);
        crate::hsd::fog::add_anim(&mut g.objs, data, Some(f), anim);
        crate::hsd::fog::req_anim(&mut g.objs, Some(f), 0.0);
        crate::hsd::fog::interpret_anim(&mut g.objs, data, Some(f));
        g.world.setup_proc(gobj, CodeRef(PROC_CUT3_FOG), 0x17);
        // The light.
        let (lg, l) = self.toy_fall_light(scene);
        let lights = data.ptr(scene.at(8)).expect("the scene's lights");
        let first = data.ptr(lights).expect("the scene's first light");
        let anims = data.ptr(first.at(4)).expect("the light's animations");
        let g = &mut *self.game;
        crate::hsd::lobj::add_anim_all(&mut g.objs, data, Some(l), data.ptr(anims));
        crate::hsd::lobj::req_anim_all(&mut g.objs, Some(l), 0.0);
        crate::hsd::lobj::anim_all(&mut g.objs, data, Some(l));
        g.world.setup_proc(lg, CodeRef(PROC_CUT3_LIGHT), 0x17);
        // The camera.
        self.toy_fall_camera(scene, CAMERA_CALLBACK, 8, 0x801, PROC_CUT3_CAMERA);
        // The background.
        let bg = self.game.endings.toy_fall.scenes[3].expect("cut 3's background");
        let m = scene_model(self, bg);
        let (gobj, j) = self.toy_fall_model(self.data.ptr(m));
        self.game.endings.toy_fall.cut3[2] = Some(gobj);
        super::super::vs::add_model_anim(&mut self.game.objs, self.data, Some(j), m, 0);
        jobj::req_anim_all(&mut self.game.objs, Some(j), 0.0);
        self.anim_all(j);
        self.game
            .world
            .setup_proc(gobj, CodeRef(PROC_CUT3_BG), 0x17);
        // The stand.
        let stand = self
            .game
            .endings
            .toy_fall
            .stand_scene
            .expect("the stand's scene");
        let joint = self.data.ptr(scene_model(self, stand));
        let (sg, sj) = self.toy_fall_model(joint);
        self.game.endings.toy_fall.cut3[1] = Some(sg);
        let m = scene_model(self, scene);
        super::super::vs::add_model_anim(&mut self.game.objs, self.data, Some(sj), m, 0);
        jobj::req_anim_all(&mut self.game.objs, Some(sj), 0.0);
        self.anim_all(sj);
        self.game
            .world
            .setup_proc(sg, CodeRef(PROC_CUT3_STAND), 0x17);
        self.toy_fall_trophies();
        // The trophy.
        let joint = self.game.endings.toy_fall.trophy;
        let (tg, tj) = self.toy_fall_model(joint);
        self.game.endings.toy_fall.cut3[0] = Some(tg);
        let trophy = self.toy_fall_trophy(self.game.endings.gover.ckind());
        let child = self.game.objs.jobjs[tj].child;
        let p = |r: &Self, f: u32| r.toy_param(trophy, f);
        let (x, y, z) = (p(self, 0), p(self, 1), p(self, 2));
        let turn = mem(fp::fmuls(reg(0.017_453_292), reg(p(self, 5))));
        let inv = fp::fdivs(1.0, reg(p(self, 4)));
        let s = mem(fp::fmuls(reg(p(self, 3)), inv));
        let child = child.expect("the original's assertion failed: jobj");
        let jobjs = &mut self.game.objs.jobjs;
        jobj::set_srt(jobjs, child, |o| o.translate.x = x);
        jobj::set_srt(jobjs, child, |o| o.translate.y = y);
        jobj::set_srt(jobjs, child, |o| o.translate.z = z);
        jobj::set_rotation_y(jobjs, child, turn);
        set_scale_each(jobjs, child, s);
        let target = descend(&self.game.objs.jobjs, sj, 3);
        self.toy_fall_follow(tj, target);
        self.game
            .world
            .setup_proc(tg, CodeRef(PROC_CUT3_TROPHY), 0x17);
    }
}

/// `GmRegEnd`'s file as the language names it: `lbArchive_LoadSymbols` completes a name
/// without an extension with the language's.
fn toy_fall_archive_name(us: bool) -> &'static str {
    if us { "GmRegEnd.usd" } else { "GmRegEnd.dat" }
}

/// The mode's trophy stands (`TyMcCmDs.dat`, `TyMcR1Ds.dat`, `TyMcR2Ds.dat`).
fn stands_file(mode: u8) -> &'static str {
    match mode {
        super::GM_CLASSIC_GOVER | GM_CLASSIC => "TyMcCmDs.dat",
        super::GM_ADVENTURE_GOVER | GM_ADVENTURE => "TyMcR1Ds.dat",
        _ => "TyMcR2Ds.dat",
    }
}

/// `n` joints down a tree's first children (`HSD_JObjGetChild` in turn).
fn descend(jobjs: &crate::hsd::jobj::JObjs, j: JObjId, n: usize) -> Option<JObjId> {
    let mut cur = Some(j);
    for _ in 0..n {
        cur = cur.and_then(|x| jobjs[x].child);
    }
    cur
}

/// `HSD_JObjSetScaleX`, `Y` and `Z` in turn.
fn set_scale_each(jobjs: &mut crate::hsd::jobj::JObjs, j: JObjId, s: f32) {
    jobj::set_srt(jobjs, j, |o| o.scale.x = s);
    jobj::set_srt(jobjs, j, |o| o.scale.y = s);
    jobj::set_srt(jobjs, j, |o| o.scale.z = s);
}

impl Running<'_> {
    /// The files the trophy fall's entry reads, for the frontend: the trophy data, the
    /// trophy, `GmRegEnd`, the stand, the mode's stands, the common items, and the
    /// character's fighters with their ending animations.
    pub fn toy_fall_scene_files(&self) -> super::super::super::vi::SceneFiles {
        let mut f = super::super::super::vi::SceneFiles::default();
        let us = self.files_us();
        // The trophy data and the trophies' models are read before the session starts.
        let ckind = self.game.endings.gover.ckind();
        let mode = self.toy_fall_mode();
        let regend = toy_fall_archive_name(us);
        f.names.push(regend.into());
        f.names.push("GmRgStnd.dat".into());
        f.names.push(stands_file(mode).into());
        f.names.push("ItCo.usd".into());
        let costume = self.game.endings.gover.color();
        f.player(0, i32::from(ckind), costume);
        f.anims(regend, i32::from(ckind), 2, 7);
        f
    }
}

/// One of cut 3's rings (`gm_801A85E4`): its first and last places in the order, the angle's
/// step in degrees and its first index's offset, the random spread and its half, the radius,
/// and the depth's squeeze.
type Ring = (i32, i32, f32, i32, f64, f64, f64, f32);
