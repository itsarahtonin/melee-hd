//! All-Star's intro (`gm_186E.c`, scene `GS_INTRO_ALLSTAR`): the character asleep (demo pose
//! 6) at the screen's joint 5 under `IrAls`' camera, light and fog, moved and scaled by its
//! character's place, for 140 ticks. No mode's states enter it; only the debug mode's state 7
//! (the Kim test's AllStar Enemy) does.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::handles::CodeRef;
use super::super::render::{FOG_CALLBACK, JOBJ_CALLBACK, LOBJ_CALLBACK};
use super::super::state::{OBJ_CAMERA, OBJ_FOG, OBJ_JOBJ, OBJ_LIGHT, Obj, Running};
use super::super::vi::Loaded;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};

/// `GS_INTRO_ALLSTAR`.
pub const GS_INTRO_ALLSTAR: u8 = 0x21;

/// The scene's entry and frame function (`gmscdata.c`'s table).
pub const ON_ENTER: CodeRef = CodeRef(0x8018_77A8);
pub const ON_FRAME: CodeRef = CodeRef(0x8018_776C);

/// The screen's model's process (`fn_80186EFC`), the fighter's (`fn_80186F6C`) and the
/// camera's render callback (`fn_80187494`).
pub const MODEL_PROC: u32 = 0x8018_6EFC;
pub const FIGHTER_PROC: u32 = 0x8018_6F6C;
pub const CAMERA_RENDER: u32 = 0x8018_7494;

/// `lbl_804D40D0` and `lbl_803D9738`: the screen's archive (completed with the saved
/// language's extension) and its scene's symbol.
const ARCHIVE: &str = "IrAls";
const SCENE_SYMBOL: &str = "ScItrAllstar_scene_data";
/// The intro's song.
const SONG: i32 = 0x2D;

/// `HSD_GOBJ_CLASS_UI`, `HSD_GOBJ_CLASS_CAMERA`, `HSD_GOBJ_CLASS_LIGHT`.
const CLASS_UI: u16 = 0xE;
const CLASS_CAMERA: u16 = 0x13;
const CLASS_LIGHT: u16 = 0xB;

/// `lbl_803D9498`: each character's place by its kind, an offset from the screen's joint 5
/// (x, y, z) and a scale (x, y, z).
const PLACES: [[f32; 6]; 28] = [
    [0.0, -4.5, 0.0, 1.0, 1.0, 1.0],
    [0.0, 1.5, 0.0, 0.6, 0.6, 0.6],
    [0.0, -5.3, 0.0, 1.3, 1.3, 1.3],
    [0.0, 1.0, 0.0, 1.0, 1.0, 1.0],
    [0.0, 5.5, 0.0, 0.75, 0.75, 0.75],
    [-1.1, -2.0, 0.0, 0.5, 0.5, 0.5],
    [-1.0, -3.5, 0.0, 1.3, 1.3, 1.3],
    [-1.0, -0.5, 0.0, 1.35, 1.35, 1.35],
    [-1.0, -2.0, 0.0, 1.4, 1.4, 1.4],
    [0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    [0.0, -6.0, 0.0, 1.0, 1.0, 1.0],
    [-1.0, -0.5, 0.0, 1.1, 1.1, 1.1],
    [-2.0, -6.5, 0.0, 1.3, 1.3, 1.3],
    [-0.5, 2.5, 0.0, 1.0, 1.0, 1.0],
    [-0.8, 5.0, 0.0, 0.7, 0.7, 0.7],
    [-1.0, 4.0, 0.0, 0.8, 0.8, 0.8],
    [0.0, -5.0, 0.0, 1.0, 1.0, 1.0],
    [0.0, 2.0, 0.0, 0.9, 0.9, 0.9],
    [-1.0, -12.5, 0.0, 1.8, 1.8, 1.8],
    [-3.5, 0.0, 0.0, 1.8, 1.8, 1.8],
    [0.0, -1.0, 0.0, 1.0, 1.0, 1.0],
    [0.0, 1.0, 0.0, 1.0, 1.0, 1.0],
    [0.0, -2.5, 0.0, 1.1, 1.1, 1.1],
    [0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    [0.0, 5.5, 0.0, 0.4, 0.4, 0.4],
    [1.0, -11.0, 0.0, 1.3, 1.3, 1.3],
    [0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    [-1.1, 2.5, 0.0, 0.4, 0.4, 0.4],
];

/// All-Star's intro's statics (`gm_186E.c`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct IntroAllstar {
    /// `lbl_804736B0.x0`: the scene's end asked for; `x4`: the screen's tick.
    pub done: i32,
    pub tick: i32,
    /// `x8`, `xC`: the screen's model's joints 5 (the fighter's place) and 6.
    pub joints: [Option<JObjId>; 2],
    /// `lbl_804D6618`: the enter data, the character and its costume.
    pub enter: [i32; 2],
    /// `lbl_804D6610`, `lbl_804D6614`: `IrAls`' archive and its scene.
    pub archive: Loaded,
    pub scene: Option<DataRef>,
}

/// Whether `scene` is All-Star's intro.
pub fn is_scene(scene: u8) -> bool {
    scene == GS_INTRO_ALLSTAR
}

impl Running<'_> {
    /// The screen's archive's name, with the saved language's extension.
    fn intro_allstar_file(&self) -> String {
        format!("{ARCHIVE}.{}", if self.files_us() { "usd" } else { "dat" })
    }

    /// The scene's enter data: only the debug mode's state 7 enters the scene.
    fn intro_allstar_enter(&self) -> [i32; 2] {
        assert!(
            self.debug_mode_scene(),
            "All-Star's intro in game mode {:#x}",
            self.game.game_mode
        );
        self.game.endings.intro_allstar.map(|v| v as i32)
    }

    /// `gm_Scene_IntroAllstar_OnEnter`: the tick and the end reset, the enter data copied, and
    /// the scene: the setups (`fn_80187714`), `IrAls`' archive, its camera, light, model
    /// (`fn_801874FC`) and fog, the fighter (`fn_801873F0`) and the intro's song.
    #[doc(alias = "gm_Scene_IntroAllstar_OnEnter")]
    pub fn intro_allstar_on_enter(&mut self) {
        // The demo fighter takes the files the preload cache holds (`lbArchive_80017040`).
        self.game.ft.data.files.from_cache = true;
        let e = self.intro_allstar_enter();
        let s = &mut self.game.intro_allstar;
        s.done = 0;
        s.tick = 0;
        s.enter = e;
        self.intro_allstar_setups();
        let name = self.intro_allstar_file();
        let (a, file, block) = self.load_archive_blocks(&name);
        let offset = self
            .data
            .get(a)
            .public(SCENE_SYMBOL)
            .unwrap_or_else(|| panic!("{SCENE_SYMBOL} isn't in {name}"));
        let scene = DataRef { archive: a, offset };
        let s = &mut self.game.intro_allstar;
        s.archive = Loaded {
            archive: Some(a),
            file,
            block,
        };
        s.scene = Some(scene);
        self.intro_allstar_camera(scene);
        self.intro_allstar_light(scene);
        self.intro_allstar_model(scene);
        self.intro_allstar_fog(scene);
        self.intro_allstar_fighter();
        self.lb_play_music(SONG);
    }

    /// `fn_80187714`: the camera's (kind 6), the dynamics', the collisions' and the dummy
    /// stage's setups (the stage set up before the items' data), the effects'.
    #[doc(alias = "fn_80187714")]
    fn intro_allstar_setups(&mut self) {
        super::super::camera::init(&mut self.game.camera, 6);
        self.game.alloc_dynamics(self.data);
        self.game.coll.reset();
        super::super::gr::setup::prepare(self.game);
        super::super::gr::setup::select(self, 0, None);
        let grkind = self.game.stage_info.grkind;
        super::super::gr::setup::setup(self, grkind);
        let kept = self.take_cached_named("ItCo.usd");
        let it_co = self.game.ft.data.files.archive("ItCo.usd");
        super::super::it::item::load_common_data(self.game, self.data, it_co, kept);
        super::super::it::item::init_globals(self.game, self.data);
        super::super::ef::eflib::init(self);
        super::super::ef::load_sync(self, 0);
    }

    /// The scene's camera, drawn by `fn_80187494` at render priority 8 over links 0 and 11.
    fn intro_allstar_camera(&mut self, scene: DataRef) {
        let data = self.data;
        let cams = data.ptr(scene.at(4)).expect("the scene's cameras");
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_CAMERA, 0x14, 0);
        let c = crate::hsd::cobj::load_desc(&mut g.objs, data, data.ptr(cams)).expect("a camera");
        g.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(gobj, CodeRef(CAMERA_RENDER), 8, false);
        g.world.gobj_mut(gobj).gxlink_prios = 0x801;
    }

    /// The scene's lights on a light GObj (`lb_80011AC4`).
    fn intro_allstar_light(&mut self, scene: DataRef) {
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_LIGHT, 3, 0);
        let lights = data.ptr(scene.at(8)).expect("the scene's lights");
        let l = super::super::gr::setup::load_light_list(g, data, lights);
        g.world
            .set_obj(gobj, OBJ_LIGHT, Obj::LObj(l.expect("a light")));
        g.world.setup_gx_link(gobj, CodeRef(LOBJ_CALLBACK), 0, 0);
    }

    /// `fn_801874FC`: the screen's model with its process (`fn_80186EFC`), its first
    /// animations started, its joints 0x3A and 0x3B put out of sight, and its joints 5 and 6
    /// kept.
    #[doc(alias = "fn_801874FC")]
    fn intro_allstar_model(&mut self, scene: DataRef) {
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_UI, 0xF, 0);
        let models = data.ptr(scene).expect("the scene's models");
        let m = data.ptr(models).expect("the screen's model");
        let j = jobj::load_joint(&mut g.objs, data, data.ptr(m)).expect("the screen's model");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 0xB, 0);
        g.world.setup_proc(gobj, CodeRef(MODEL_PROC), 0);
        super::vs::add_model_anim(&mut g.objs, data, Some(j), m, 0);
        jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
        self.anim_all(j);
        let jobjs = &mut self.game.objs.jobjs;
        for index in [0x3A, 0x3B] {
            let n = super::pause::tree_nodes(jobjs, Some(j), &[index])[0];
            let n = n.expect("a hidden joint");
            jobj::set_srt(jobjs, n, |o| o.translate.z = 10000.0);
        }
        let [j5, j6] = super::pause::tree_nodes(jobjs, Some(j), &[5, 6]);
        self.game.intro_allstar.joints = [j5, j6];
    }

    /// The scene's fog.
    fn intro_allstar_fog(&mut self, scene: DataRef) {
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_UI, 0xF, 0);
        let fogs = data.ptr(scene.at(0xC)).expect("the scene's fogs");
        let desc = data.ptr(fogs).expect("the scene's fog");
        let f = crate::hsd::fog::load_desc(&mut g.objs.fogs, data, desc);
        g.world.set_obj(gobj, OBJ_FOG, Obj::Fog(f));
        g.world.setup_gx_link(gobj, CodeRef(FOG_CALLBACK), 0xB, 0);
    }

    /// `fn_801873F0`: the fighters' setup for demos, the players reset, the character's intro
    /// animations from `IrAls`, and slot 0's demo player (the character and costume, port 0,
    /// facing 0, size 1) asleep (pose 6) with its place's process (`fn_80186F6C`).
    #[doc(alias = "fn_801873F0")]
    fn intro_allstar_fighter(&mut self) {
        self.fighter_demo_init();
        self.game.init_all_players();
        let s = self.game.intro_allstar;
        let archive = s.archive.archive.expect("the screen's archive");
        let [ckind, costume] = s.enter;
        self.player_set_demo_archive_idx(ckind, archive, s.archive.file, 1);
        self.game.set_player_ckind(0, ckind);
        self.game.set_player_costume(0, costume);
        self.game.set_player_port(0, 0);
        self.game
            .set_player_pkind(0, super::super::pl::player::PKIND_DEMO);
        self.game.set_player_facing(0, 0.0);
        self.game.set_player_model_scale(0, 1.0);
        self.player_create_demo(0, 6);
        let g = self.game.player_entity(0).expect("a demo fighter");
        self.game.world.setup_proc(g, CodeRef(FIGHTER_PROC), 0x16);
    }

    /// `gm_Scene_IntroAllstar_OnFrame`: once the screen asked to end, `IrAls` freed, the
    /// music stopped and the scene's end.
    #[doc(alias = "gm_Scene_IntroAllstar_OnFrame")]
    pub fn intro_allstar_on_frame(&mut self) {
        let s = self.game.intro_allstar;
        if s.done != 0 {
            self.free_archive_blocks(s.archive.file, s.archive.block);
            self.lb_audio_music_stop();
            self.game.end_scene();
        }
    }

    /// `fn_80186EFC`, the screen's model's process: its joint 6's animation back at frame 0,
    /// its animations; its tick counted to 140, then the scene's end asked for.
    #[doc(alias = "fn_80186EFC")]
    pub fn intro_allstar_model_proc(&mut self, gobj: GObjId) {
        let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj else {
            panic!("All-Star's intro's model holds no JObj");
        };
        let j6 = self.game.intro_allstar.joints[1];
        jobj::req_anim_all(&mut self.game.objs, j6, 0.0);
        self.anim_all(j);
        let s = &mut self.game.intro_allstar;
        if s.tick < 0x8C {
            s.tick += 1;
        } else {
            s.done = 1;
        }
    }

    /// `fn_80186F6C`, the fighter's process: its root at the screen's joint 5, 7 to the left,
    /// moved and scaled by its character's place; its slot's other fighter (Nana) 3 right, 4
    /// up and 5 back from it at its scale.
    #[doc(alias = "fn_80186F6C")]
    pub fn intro_allstar_fighter_proc(&mut self, gobj: GObjId) {
        let root = self.root_jobj(gobj);
        let s = self.game.intro_allstar;
        let src = s.joints[0].expect("the fighter's place");
        let ckind = s.enter[0];
        let Some(p) = usize::try_from(ckind).ok().and_then(|k| PLACES.get(k)) else {
            super::super::code::unported_call(&format!(
                "All-Star's intro's place of character {ckind} (past lbl_803D9498's end)"
            ))
        };
        let jobjs = &mut self.game.objs.jobjs;
        let t = jobjs[src].translate;
        let x = fp::fsubs(reg(t.x), 7.0);
        let x = mem(fp::fadds(reg(mem(x)), reg(p[0])));
        let y = mem(fp::fadds(reg(t.y), reg(p[1])));
        let z = mem(fp::fadds(reg(t.z), reg(p[2])));
        jobj::set_srt(jobjs, root, |o| {
            o.translate.x = x;
            o.translate.y = y;
            o.translate.z = z;
        });
        jobj::set_srt(jobjs, root, |o| o.scale.x = p[3]);
        jobj::set_srt(jobjs, root, |o| o.scale.y = p[4]);
        jobj::set_srt(jobjs, root, |o| o.scale.z = p[5]);
        if let Some(e2) = self.game.player_entity_at(0, 1) {
            let j2 = self.root_jobj(e2);
            let jobjs = &mut self.game.objs.jobjs;
            let (t, sc) = (jobjs[root].translate, jobjs[root].scale);
            let x = mem(fp::fadds(reg(t.x), 3.0));
            let z = mem(fp::fsubs(reg(t.z), 5.0));
            let y = mem(fp::fadds(reg(t.y), 4.0));
            jobj::set_srt(jobjs, j2, |o| {
                o.translate.x = x;
                o.translate.y = y;
                o.translate.z = z;
            });
            jobj::set_srt(jobjs, j2, |o| o.scale = sc);
        }
    }

    /// `fn_80187494`, the camera's render callback: through the camera, the fighters' passes
    /// (`Camera_800313E0`), then render links 0 and 11.
    #[doc(alias = "fn_80187494")]
    pub fn intro_allstar_camera_render(&mut self, gobj: GObjId) {
        let c = match self.game.world.gobj(gobj).obj {
            Some(Obj::CObj(c)) => Some(c),
            other => panic!("All-Star's intro's camera holds {other:?}"),
        };
        if crate::hsd::display::camera_begin(&mut self.game.objs, self.data, c) {
            self.camera_fighters(gobj, 1);
            self.game.world.gobj_mut(gobj).gxlink_prios = 0x801;
            self.render_links(gobj, 7);
            crate::hsd::display::camera_end(&mut self.game.objs);
        }
    }

    /// The files the intro about to enter reads, for the frontend: `IrAls`, the demo fighter
    /// (its character and costume) and its intro animations, and the song.
    pub fn intro_allstar_scene_files(&self) -> Option<super::super::vi::SceneFiles> {
        if !is_scene(self.entering_scene()?) {
            return None;
        }
        let mut f = super::super::vi::SceneFiles::default();
        let archive = self.intro_allstar_file();
        f.names.push(archive.clone());
        f.songs.push(SONG as usize);
        let [ckind, costume] = self.intro_allstar_enter();
        f.player(0, ckind, costume as u8);
        let m = super::super::pl::player::mapping(ckind);
        for kind in [m.internal_id, m.extra_internal_id] {
            if kind < 0 || (kind == m.extra_internal_id && m.has_transformation != 0) {
                continue;
            }
            f.anims.push((archive.clone(), i32::from(kind), 1, 10, 11));
        }
        Some(f)
    }
}

/// Runs the intro's process, if `f` is one.
pub fn run_proc(r: &mut Running, gobj: GObjId, f: u32) -> bool {
    match f {
        MODEL_PROC => r.intro_allstar_model_proc(gobj),
        FIGHTER_PROC => r.intro_allstar_fighter_proc(gobj),
        _ => return false,
    }
    true
}

/// Runs the intro's frame function, if `f` is one.
pub fn run_frame(r: &mut Running, f: u32) -> bool {
    if f != ON_FRAME.0 {
        return false;
    }
    r.intro_allstar_on_frame();
    true
}

/// Runs the intro's render callback, if `cb` is one.
pub fn run_render(r: &mut Running, gobj: GObjId, cb: u32) -> bool {
    if cb != CAMERA_RENDER {
        return false;
    }
    r.intro_allstar_camera_render(gobj);
    true
}
