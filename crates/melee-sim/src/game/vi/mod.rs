//! The cutscenes (`melee/vi`): Adventure's eleven scenes between its stages, each a model
//! scene of its own (`Vi*.dat`) with an animated camera, lights and fog, most on a stage set
//! up for a demo (`Stage_8022532C`) with demo fighters in the cutscene's motions, ended by the
//! camera's animation or by Start. The debug cutscene mode (`gm_Mode_DebugCutscene_States`,
//! mode 0x19, `gmopeningmode.c`) plays them in turn with random characters, then
//! congratulations. `vi.c` holds what they share: the characters' wait animations, the
//! rumble, the camera's passes and Start's skip.

pub mod v0102;
pub mod v0401;
pub mod v0402;
pub mod v0501;
pub mod v0502;
pub mod v0601;
pub mod v0801;

use melee_data::archive::{ArchiveId, DataRef};

use super::handles::CodeRef;
use super::render::{FOG_CALLBACK, JOBJ_CALLBACK, LOBJ_CALLBACK};
use super::state::{OBJ_CAMERA, OBJ_FOG, OBJ_JOBJ, OBJ_LIGHT, Obj, Running};
use crate::hsd::fog;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};

/// `GM_DEBUG_CUTSCENE`.
pub const GM_DEBUG_CUTSCENE: u8 = 0x19;

/// `GS_REGEND_CONGRATS`, the scene after the last cutscene.
pub const GS_REGEND_CONGRATS: u8 = 0x10;

/// `HSD_GOBJ_CLASS_UI`, `HSD_GOBJ_CLASS_CAMERA`, `HSD_GOBJ_CLASS_LIGHT` (the fog's too).
const CLASS_UI: u16 = 0xE;
const CLASS_CAMERA: u16 = 0x13;
const CLASS_LIGHT: u16 = 0xB;

/// Each cutscene's scene (`GS_CUTSCENE_LUIGI` to `GS_CUTSCENE_GIGADEFEATED`) and its entry
/// and frame functions (`gmscdata.c`'s table).
pub const SCENES: [(u8, u32, u32); 11] = [
    (0x11, 0x8031_CD94, 0x8031_D000),
    (0x12, 0x8031_D288, 0x8031_D698),
    (0x13, 0x8031_D858, 0x8031_D9C4),
    (0x14, 0x8031_DE58, 0x8031_E0F0),
    (0x15, 0x8031_E444, 0x8031_E6CC),
    (0x16, 0x8031_EBBC, 0x8031_ED50),
    (0x17, 0x8031_F07C, 0x8031_F274),
    (0x18, 0x8031_F714, 0x8031_F960),
    (0x19, 0x8031_FD18, 0x8032_0490),
    (0x1A, 0x8032_0A40, 0x8032_10EC),
    (0x1B, 0x8032_1300, 0x8032_18E0),
];

/// A cutscene scene's entry and frame functions.
pub fn scene_handlers(scene: u8) -> Option<(CodeRef, CodeRef)> {
    SCENES
        .iter()
        .find(|s| s.0 == scene)
        .map(|s| (CodeRef(s.1), CodeRef(s.2)))
}

/// `vi_animation_table`: each character's wait animations for the cutscenes, by its
/// character kind (`viGetCharAnimByIndex`).
pub const WAIT_FILES: [&str; 26] = [
    "PlCaDViWaitAJ.dat",
    "PlDkDViWaitAJ.dat",
    "PlFxDViWaitAJ.dat",
    "PlGwDViWaitAJ.dat",
    "PlKbDViWaitAJ.dat",
    "PlKpDViWaitAJ.dat",
    "PlLkDViWaitAJ.dat",
    "PlLgDViWaitAJ.dat",
    "PlMrDViWaitAJ.dat",
    "PlMsDViWaitAj.dat",
    "PlMtDViWaitAJ.dat",
    "PlNsDViWaitAJ.dat",
    "PlPeDViWaitAJ.dat",
    "PlPkDViWaitAJ.dat",
    "PlPpDViWaitAJ.dat",
    "PlPrDViWaitAJ.dat",
    "PlSsDViWaitAJ.dat",
    "PlYsDViWaitAJ.dat",
    "PlZdDViWaitAJ.dat",
    "PlSkDViWaitAJ.dat",
    "PlFcDViWaitAJ.dat",
    "PlClDViWaitAJ.dat",
    "PlDrDViWaitAJ.dat",
    "PlFeDViWaitAJ.dat",
    "PlPcDViWaitAJ.dat",
    "PlGnDViWaitAJ.dat",
];

/// A character's wait animations' file, `viGetCharAnimByIndex`.
#[doc(alias = "viGetCharAnimByIndex")]
pub fn wait_file(ckind: u8) -> &'static str {
    WAIT_FILES[usize::from(ckind)]
}

/// An archive a cutscene loads into the scene's heap (`lbArchive_LoadSymbols`): its archive
/// and its two blocks (its bytes, its `HSD_Archive`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Loaded {
    pub archive: Option<ArchiveId>,
    pub file: u32,
    pub block: u32,
}

/// The cutscenes' statics.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Cutscenes {
    /// Each cutscene's enter data (`ViCharaDesc`, eight bytes of it): `un_804D6F3C`
    /// (`vi0102`'s), `un_804D6F60` (`vi0401`'s), `un_804D6F84` (`vi0501`'s), `un_804D6FA8`
    /// (`vi0502`'s), `un_804D6FD8` (`vi1101`'s), `un_804D7004` (`vi1201v1`'s), `un_804D7038`
    /// (`vi1201v2`'s).
    pub desc_0102: [u8; 8],
    pub desc_0401: [u8; 8],
    pub desc_0501: [u8; 8],
    pub desc_0502: [u8; 8],
    pub desc_1101: [u8; 8],
    pub desc_1201v1: [u8; 8],
    pub desc_1201v2: [u8; 8],
    pub v0102: v0102::Vi0102,
    pub v0401: v0401::Vi0401,
    pub v0402: v0402::Vi0402,
    pub v0501: v0501::Vi0501,
    pub v0502: v0502::Vi0502,
    pub v0601: v0601::Vi0601,
    pub v0801: v0801::Vi0801,
}

/// A scene's `SceneDesc` from its archive's symbol.
fn scene_symbol(r: &Running, a: ArchiveId, name: &str, sym: &str) -> DataRef {
    let offset = r
        .data
        .get(a)
        .public(sym)
        .unwrap_or_else(|| panic!("{sym} isn't in {name}"));
    DataRef { archive: a, offset }
}

/// `SceneDesc::models[i]`, none past the list's end.
fn scene_model(r: &Running, scene: DataRef, i: u32) -> Option<DataRef> {
    let models = r.data.ptr(scene)?;
    r.data.ptr(models.at(4 * i))
}

/// `SceneDesc::cameras[0]`'s `desc` and its first animation.
fn scene_camera(r: &Running, scene: DataRef) -> (DataRef, Option<DataRef>) {
    let cams = r.data.ptr(scene.at(4)).expect("the scene's cameras");
    let desc = r.data.ptr(cams).expect("the scene's camera");
    let anims = r.data.ptr(cams.at(4));
    (desc, anims.and_then(|a| r.data.ptr(a)))
}

/// `SceneDesc::lights`.
fn scene_lights(r: &Running, scene: DataRef) -> DataRef {
    r.data.ptr(scene.at(8)).expect("the scene's lights")
}

/// `SceneDesc::fogs[0].desc`.
fn scene_fog(r: &Running, scene: DataRef) -> DataRef {
    let fogs = r.data.ptr(scene.at(0xC)).expect("the scene's fogs");
    r.data.ptr(fogs).expect("the scene's fog")
}

impl Running<'_> {
    /// An archive into the scene's heap, `lbArchive_LoadSymbols`.
    fn vi_load(&mut self, name: &str) -> Loaded {
        let (a, file, block) = self.load_archive_blocks(name);
        Loaded {
            archive: Some(a),
            file,
            block,
        }
    }

    /// The scene's camera, as each cutscene makes it: [`Running::vi_camera_unanimated`],
    /// then [`Running::vi_camera_animate`].
    fn vi_camera(&mut self, scene: DataRef, render: u32, link: u8, proc: u32) -> GObjId {
        let gobj = self.vi_camera_unanimated(scene, render, link);
        self.vi_camera_animate(gobj, scene, proc);
        gobj
    }

    /// A camera GObj (p-link 20) with the scene's first camera (`lb_80013B14`) drawn by
    /// `render` on render link `link`.
    fn vi_camera_unanimated(&mut self, scene: DataRef, render: u32, link: u8) -> GObjId {
        let (desc, _) = scene_camera(self, scene);
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_CAMERA, 20, 0);
        let c =
            crate::lb::cobj::load_cobj(&mut g.objs, data, crate::lb::cobj::CObjDesc::Data(desc));
        g.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(gobj, CodeRef(render), link, false);
        gobj
    }

    /// The camera's animation, the scene's first camera's first, added and started
    /// (`HSD_CObjAddAnim`, `HSD_CObjReqAnim`, `HSD_CObjAnim`), and its process `proc`.
    fn vi_camera_animate(&mut self, gobj: GObjId, scene: DataRef, proc: u32) {
        let (_, anim) = scene_camera(self, scene);
        let c = self.vi_cobj(gobj);
        let data = self.data;
        let g = &mut *self.game;
        crate::hsd::cobj::add_anim(&mut g.objs, data, Some(c), anim);
        crate::hsd::cobj::req_anim(&mut g.objs, Some(c), 0.0);
        crate::hsd::cobj::anim(&mut g.objs, data, Some(c));
        g.world.setup_proc(gobj, CodeRef(proc), 0);
    }

    /// A camera GObj's CObj.
    fn vi_cobj(&self, gobj: GObjId) -> crate::hsd::cobj::CObjId {
        match self.game.world.gobj(gobj).obj {
            Some(Obj::CObj(c)) => c,
            other => panic!("a cutscene's camera holds {other:?}"),
        }
    }

    /// A camera's process that ends the scene at its animation's last frame (`vi_8031D80C`
    /// and its kin): its animation, then the CObj's own frame against its end.
    fn vi_camera_anim_ends(&mut self, gobj: GObjId) {
        let c = self.vi_cobj(gobj);
        crate::hsd::cobj::anim(&mut self.game.objs, self.data, Some(c));
        let (cur, end) = self.vi_cobj_frame(c);
        if cur == end {
            self.vi_end();
        }
    }

    /// The current and last frames of a camera's own animation.
    fn vi_cobj_frame(&self, c: crate::hsd::cobj::CObjId) -> (f32, f32) {
        let objs = &self.game.objs;
        let a = objs.cobjs[c].aobj.expect("a camera without an animation");
        (objs.aobjs[a].curr_frame, objs.aobjs[a].end_frame)
    }

    /// The scene's models, each a UI GObj (p-link 15) drawn on render link `link`, its first
    /// animations added (`gm_8016895C`), started at frame 0 and run once, and its process
    /// `proc` (priority 0x17); `each` sees each model's tree.
    fn vi_models(
        &mut self,
        scene: DataRef,
        link: u8,
        proc: u32,
        mut each: impl FnMut(&mut Self, u32, JObjId),
    ) {
        let mut i = 0;
        while let Some(m) = scene_model(self, scene, i) {
            let j = self.vi_model(m, link, proc);
            each(self, i, j);
            i += 1;
        }
    }

    /// One of the scene's models (its `DynamicModelDesc` `m`): a UI GObj (p-link 15) drawn on
    /// render link `link`, its first animations added (`gm_8016895C`), started at frame 0 and
    /// run once, and its process `proc` (priority 0x17).
    fn vi_model(&mut self, m: DataRef, link: u8, proc: u32) -> JObjId {
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_UI, 15, 0);
        let j = jobj::load_joint(&mut g.objs, data, data.ptr(m)).expect("a scene model");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), link, 0);
        super::gm::vs::add_model_anim(&mut g.objs, data, Some(j), m, 0);
        jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
        self.anim_all(j);
        self.game.world.setup_proc(gobj, CodeRef(proc), 0x17);
        j
    }

    /// The scene's fog (`HSD_FogLoadDesc` on a GObj of class `class`, p-link 3), whose color
    /// the camera erases the screen to.
    fn vi_fog_with(&mut self, scene: DataRef, class: u16) -> [u8; 4] {
        let desc = scene_fog(self, scene);
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(class, 3, 0);
        let f = fog::load_desc(&mut g.objs.fogs, data, desc);
        g.world.set_obj(gobj, OBJ_FOG, Obj::Fog(f));
        g.world.setup_gx_link(gobj, CodeRef(FOG_CALLBACK), 0, 0);
        g.objs.fogs[f].color
    }

    /// Whether Adventure's run stands at its 0x14th stage (`gm_8017E7E0`: Adventure's data's
    /// `x7C`), which picks the Star Fox cutscene's later talks.
    #[doc(alias = "gm_8017E7E0")]
    pub(crate) fn adventure_starfox_lost(&self) -> bool {
        self.game.one_player.adventure.x7c == 0x14
    }

    /// The current and last frames of a camera's eye's animation.
    fn vi_eye_frame(&self, c: crate::hsd::cobj::CObjId) -> (f32, f32) {
        let objs = &self.game.objs;
        let eye = objs.cobjs[c].eyepos.expect("a camera without an eye");
        let a = objs.wobjs[eye.0].aobj.expect("an eye without an animation");
        (objs.aobjs[a].curr_frame, objs.aobjs[a].end_frame)
    }

    /// The scene's lights (`lb_80011AC4` on a light GObj, p-link 3).
    fn vi_lights(&mut self, scene: DataRef) {
        let list = scene_lights(self, scene);
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_LIGHT, 3, 0);
        let l = super::gr::setup::load_light_list(g, data, list);
        g.world
            .set_obj(gobj, OBJ_LIGHT, Obj::LObj(l.expect("a light")));
        g.world.setup_gx_link(gobj, CodeRef(LOBJ_CALLBACK), 0, 0);
    }

    /// `Stage_InitScene` (with `Ground_InitScene`), then the items' and the stage's setup as
    /// every cutscene on a stage runs them: the camera's (`Camera_Init(6)`), the dynamics',
    /// collisions' and stage's resets, the stage `stkind` loaded, the common items
    /// (`Item_80266FA8`, `Item_80266FCC`); `before_setup` runs before the stage's setup
    /// (`Stage_8022524C`), then its demo setup (`Stage_8022532C`).
    fn vi_stage(&mut self, stkind: i32, before_setup: impl FnOnce(&mut Self)) {
        super::camera::init(&mut self.game.camera, 6);
        self.game.alloc_dynamics(self.data);
        self.game.coll.reset();
        super::gr::setup::prepare(self.game);
        let grkind = crate::lb::audio::ground_kind(stkind) as i32;
        let us = self.files_us();
        let archive = super::gr::data::stage_file(grkind, us).map(|name| {
            self.game
                .ft
                .data
                .files
                .archive(name.trim_start_matches('/'))
        });
        super::gr::setup::select(self, stkind, archive);
        let kept = self.take_cached_named("ItCo.usd");
        let it_co = self.game.ft.data.files.archive("ItCo.usd");
        super::it::item::load_common_data(self.game, self.data, it_co, kept);
        super::it::item::init_globals(self.game, self.data);
        before_setup(self);
        let grkind = self.game.stage_info.grkind;
        super::gr::setup::setup(self, grkind);
        super::gr::stage::demo_init(self, stkind, 0);
    }

    /// A demo player as the cutscenes set one up: its character's cutscene animations from
    /// `archive` (`Player_80036E20`'s `idx`), its character, costume, port 0, the demo slot
    /// type, facing `facing`, at `pos`, then (in `create`) its fighters in pose `pose`.
    #[allow(clippy::too_many_arguments)]
    fn vi_player(
        &mut self,
        slot: i32,
        ckind: i32,
        costume: i32,
        archive: Loaded,
        idx: i32,
        facing: f32,
        pos: Option<crate::vec::Vec3>,
    ) {
        self.player_set_demo_archive_idx(
            ckind,
            archive.archive.expect("a cutscene's archive"),
            archive.file,
            idx,
        );
        self.game.set_player_ckind(slot, ckind);
        self.game.set_player_costume(slot, costume);
        self.game.set_player_port(slot, 0);
        self.game
            .set_player_pkind(slot, super::pl::player::PKIND_DEMO);
        self.game.set_player_facing(slot, facing);
        if let Some(p) = pos {
            self.game.set_player_pos(slot, p);
        }
    }

    /// The cutscenes' rumble on Adventure's player's port, `vi_8031C9B4`: none when its
    /// rumble is off (`gm_8017E440`'s 4).
    #[doc(alias = "vi_8031C9B4")]
    fn vi_rumble(&mut self, kind: i32, frames: i32) {
        let port = self.adventure_rumble_port();
        if port != 4 {
            self.lot_rumble(port, kind, frames);
        }
    }

    /// Adventure's player's slot (`gm_8017E430`), whose Start skips a cutscene.
    #[doc(alias = "gm_8017E430")]
    fn adventure_slot(&self) -> u8 {
        self.game.one_player.adventure.base.player.slot
    }

    /// Adventure's player's port when its rumble is on, else 4 (`gm_8017E440`).
    #[doc(alias = "gm_8017E440")]
    fn adventure_rumble_port(&mut self) -> u8 {
        let p = self.game.one_player.adventure.base.player;
        if self.rumble_enabled_for(usize::from(p.slot), p.nametag) {
            p.slot
        } else {
            4
        }
    }

    /// A camera's passes as the cutscenes draw them, `vi_8031CA04`: the game camera's layers
    /// 2, 1 and 0 over render links 0 and 3 (then 3 twice), and the fighters' passes
    /// (`Camera_800313E0`).
    #[doc(alias = "vi_8031CA04")]
    fn vi_camera_passes(&mut self, gobj: GObjId) {
        for (layer, prios) in [(2, 9), (1, 8), (0, 8)] {
            super::camera::set_layer(&mut self.game.camera.cam, layer);
            self.game.world.gobj_mut(gobj).gxlink_prios = prios;
            self.render_links(gobj, 7);
        }
        self.camera_fighters(gobj, 1);
    }

    /// `vi_RunCamera`: through the camera, the screen erased to `erase`, the cutscene's
    /// passes, then render links `prios`.
    pub(crate) fn vi_run_camera(&mut self, gobj: GObjId, erase: [u8; 4], prios: u64) {
        let c = match self.game.world.gobj(gobj).obj {
            Some(Obj::CObj(c)) => Some(c),
            other => panic!("a cutscene's camera holds {other:?}"),
        };
        if crate::hsd::display::camera_begin(&mut self.game.objs, self.data, c) {
            let draws = &mut self.game.objs.display.draws;
            draws.push(crate::hsd::display::Draw::EraseColor(erase));
            draws.push(crate::hsd::display::Draw::EraseWithDepth);
            self.vi_camera_passes(gobj);
            self.game.world.gobj_mut(gobj).gxlink_prios = prios;
            self.render_links(gobj, 7);
            crate::hsd::display::camera_end(&mut self.game.objs);
        }
    }

    /// Start on Adventure's player's port ends the cutscene, `vi_8031CAAC`: the music and
    /// sounds stopped, the rumble too.
    #[doc(alias = "vi_8031CAAC")]
    #[doc(alias = "vi0102_Scene_OnFrame")]
    #[doc(alias = "vi0402_Scene_OnFrame")]
    #[doc(alias = "vi0501_Scene_OnFrame")]
    #[doc(alias = "vi0502_Scene_OnFrame")]
    #[doc(alias = "vi0601_Scene_OnFrame")]
    fn vi_skip(&mut self) {
        let slot = self.adventure_slot();
        let input = self.game.buttons_triggered(slot);
        if input & u64::from(crate::input::pad::BUTTON_START) != 0 {
            self.lb_audio_music_stop();
            self.sfx_stop_all();
            self.rumble_stop_all();
            self.game.end_scene();
        }
    }

    /// The camera's animation reached its end: the rumble stopped and the scene ended.
    fn vi_end(&mut self) {
        self.rumble_stop_all();
        self.game.end_scene();
    }

    /// A model GObj's process that runs its animations (`vi0102_JObjCallback` and its kin).
    fn vi_anim_proc(&mut self, gobj: GObjId) {
        let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj else {
            panic!("a cutscene model holds no JObj");
        };
        self.anim_all(j);
    }

    /// A cutscene scene's entry.
    pub fn vi_on_enter(&mut self, scene: u8) {
        // The scene takes the files the preload cache holds (`lbArchive_80017040`): the
        // common effects and items.
        self.game.ft.data.files.from_cache = true;
        match scene {
            0x11 => self.vi0102_on_enter(),
            0x12 => self.vi0401_on_enter(),
            0x13 => self.vi0402_on_enter(),
            0x14 => self.vi0501_on_enter(),
            0x15 => self.vi0502_on_enter(),
            0x16 => self.vi0601_on_enter(),
            0x17 => self.vi0801_on_enter(),
            s => super::code::unported_call(&format!("the cutscene scene {s:#x}'s entry")),
        }
    }

    /// The debug cutscene mode's last state's exit, `onExitRegendCongrats`: the title next.
    // Ports onExitRegendCongrats; aliased once a check reaches it (congratulations, task 5).
    pub fn debug_cutscene_exit(&mut self) {
        self.set_pending_game_mode(super::gm::machine::GM_TITLE);
        self.set_new_game_mode_pending();
    }

    /// The debug cutscene mode's first state's entry, `onEnterCutsceneLuigi`: Adventure's
    /// cutscenes' enter data with the Ice Climbers in random costumes, the Kirbys' costumes
    /// and counts at random, and congratulations' character (Mario) and second value.
    #[doc(alias = "onEnterCutsceneLuigi")]
    #[doc(alias = "vi0401_8031D020")]
    #[doc(alias = "un_8031D9E4")]
    #[doc(alias = "un_8031E110")]
    #[doc(alias = "un_8031F980")]
    #[doc(alias = "un_803204B0")]
    #[doc(alias = "gm_801BEFA4")]
    #[doc(alias = "gm_801BEFC0")]
    pub fn debug_cutscene_enter(&mut self) {
        const ICS: u8 = 0xE;
        let rand = |r: &mut Self, n: i32| r.game.objs.rng.below(n) as u8;
        let v = &mut self.game.vi;
        v.desc_0102[1] = 1;
        v.desc_0102[3] = 1;
        let c = rand(self, 4);
        let v = &mut self.game.vi;
        v.desc_0401[0] = ICS;
        v.desc_0401[1] = c;
        // `un_8031D9E4(0xE, HSD_Randi(4), temp_r31)`: the second draw first.
        let kirbys = rand(self, 4);
        let c = rand(self, 4);
        let v = &mut self.game.vi;
        v.desc_0501[0] = ICS;
        v.desc_0501[1] = c;
        v.desc_0501[3] = kirbys;
        for i in 4..7 {
            let n = rand(self, 0x1B);
            self.game.vi.desc_0501[i] = n;
        }
        let kirby = rand(self, 4);
        let c = rand(self, 4);
        let v = &mut self.game.vi;
        v.desc_0502[0] = ICS;
        v.desc_0502[1] = c;
        v.desc_0502[3] = kirby;
        let c = rand(self, 4);
        let v = &mut self.game.vi;
        v.desc_1101[0] = ICS;
        v.desc_1101[1] = c;
        let c = rand(self, 4);
        let v = &mut self.game.vi;
        v.desc_1201v1[0] = ICS;
        v.desc_1201v1[1] = c;
        let c = rand(self, 4);
        let v = &mut self.game.vi;
        v.desc_1201v2[0] = ICS;
        v.desc_1201v2[1] = c;
        self.game.endings.gover.set_ckind(8);
        let c = rand(self, 4);
        self.game.endings.gover.set_color(c);
    }
}

/// Runs a cutscene's process or frame function, if `f` is one.
pub fn run_proc(r: &mut Running, gobj: GObjId, f: u32) -> bool {
    match f {
        v0102::JOBJ_PROC => r.vi_anim_proc(gobj),
        v0102::CAMERA_PROC => r.vi0102_camera_proc(gobj),
        v0401::INFO_PROC | v0401::JOBJ_PROC | v0402::JOBJ_PROC | v0502::JOBJ_PROC => {
            r.vi_anim_proc(gobj)
        }
        v0401::CAMERA_PROC | v0402::CAMERA_PROC => r.vi_camera_anim_ends(gobj),
        v0501::CAMERA_PROC => r.vi0501_camera_proc(gobj),
        v0502::CAMERA_PROC => r.vi0502_camera_proc(gobj),
        v0601::JOBJ_PROC => r.vi_anim_proc(gobj),
        v0601::ARWING_PROC => r.vi0601_arwing_proc(gobj),
        v0601::CAMERA_PROC => r.vi0601_camera_proc(gobj),
        v0801::JOBJ_PROC => r.vi_anim_proc(gobj),
        v0801::CAMERA_PROC => r.vi0801_camera_proc(gobj),
        _ => return false,
    }
    true
}

/// Runs a cutscene's frame function, if `f` is one.
pub fn run_frame(r: &mut Running, f: u32) -> bool {
    match f {
        0x8031_D000 | 0x8031_D9C4 | 0x8031_E0F0 | 0x8031_E6CC | 0x8031_ED50 | 0x8031_F274 => {
            r.vi_skip()
        }
        v0401::ON_FRAME => r.vi0401_on_frame(),
        _ => return false,
    }
    true
}

/// Runs a cutscene's render callback, if `cb` is one.
pub fn run_render(r: &mut Running, gobj: GObjId, cb: u32, _pass: i32) -> bool {
    match cb {
        v0102::CAMERA_RENDER => r.vi0102_camera_render(gobj),
        v0401::CAMERA_RENDER => r.vi0401_camera_render(gobj),
        v0601::CAMERA_RENDER => r.vi0601_camera_render(gobj),
        v0801::CAMERA_RENDER => r.vi0801_camera_render(gobj),
        v0501::CAMERA_RENDER => {
            r.shadow_render(false);
            let erase = r.game.vi.v0501.erase;
            r.vi_run_camera(gobj, erase, 0x281);
        }
        v0502::CAMERA_RENDER => {
            r.shadow_render(false);
            let erase = r.game.vi.v0502.erase;
            r.vi_run_camera(gobj, erase, 0x281);
        }
        _ => return false,
    }
    true
}

/// What a cutscene's scene reads from the disc, for the frontend (a session's console) to
/// read before its entry: its files by name, its demo players' fighters (as a match's start
/// data names them), and the demo animations its fighters take, each `(file, kind, idx, lo,
/// hi)` as `Player_80036E20` and the pose name them.
#[derive(Debug, Default)]
pub struct SceneFiles {
    pub names: Vec<String>,
    pub players: super::structs::StartMeleeData,
    pub anims: Vec<(String, i32, i32, usize, usize)>,
    /// The songs the scene's entry starts, which the audio driver reads during it.
    pub songs: Vec<usize>,
    /// The copies Kirbys carry in (`Player_80031DA8`'s kind and costume).
    pub copies: Vec<(usize, usize)>,
}

impl SceneFiles {
    fn player(&mut self, slot: usize, ckind: i32, costume: u8) {
        let p = &mut self.players.players[slot];
        p.ckind = ckind as i8;
        p.color = costume;
        p.slot_type = super::pl::player::PKIND_DEMO as u8;
    }

    /// A player's demo animations from `file` (`Player_80036E20`'s `idx`) for `pose`: its
    /// character's kinds' (both Ice Climbers).
    fn anims(&mut self, file: &str, ckind: i32, idx: i32, pose: i32) {
        let m = super::pl::player::mapping(ckind);
        for kind in [m.internal_id, m.extra_internal_id] {
            if kind < 0 || (kind == m.extra_internal_id && m.has_transformation != 0) {
                continue;
            }
            let kind = i32::from(kind);
            let (lo, hi) = match pose {
                p if p >= 9 => super::ft::demo::cutscene_anims(kind, p),
                8 => (13, 13),
                p => panic!("a cutscene's pose {p}"),
            };
            self.anims.push((file.to_owned(), kind, idx, lo, hi));
        }
    }
}

impl Running<'_> {
    /// The files the cutscene scene about to enter reads, none for another scene.
    pub fn vi_scene_files(&self) -> Option<SceneFiles> {
        let scene = self.entering_scene()?;
        scene_handlers(scene)?;
        let mut f = SceneFiles::default();
        let us = self.files_us();
        let stage = |f: &mut SceneFiles, stkind: i32| {
            let grkind = crate::lb::audio::ground_kind(stkind) as i32;
            if let Some(name) = super::gr::data::stage_file(grkind, us) {
                f.names.push(name.trim_start_matches('/').to_owned());
            }
        };
        let v = &self.game.vi;
        match scene {
            0x11 => {
                f.names.push("Vi0102.dat".into());
                f.songs.push(0x56);
                stage(&mut f, 4);
                f.player(0, 8, v.desc_0102[1]);
                f.player(1, 7, v.desc_0102[3]);
                f.anims("Vi0102.dat", 8, 4, 9);
                f.anims("Vi0102.dat", 7, 4, 9);
            }
            0x12 => {
                let (ck, waits) = (i32::from(v.desc_0401[0]), wait_file(v.desc_0401[0]));
                f.names
                    .extend(["Vi0401.dat", "IfAll.dat", waits].map(String::from));
                stage(&mut f, 6);
                f.player(0, ck, v.desc_0401[1]);
                f.anims(waits, ck, 3, 8);
            }
            0x13 => {
                f.names.push("Vi0402.dat".into());
                f.songs.push(0x58);
            }
            0x14 => {
                let (ck, waits) = (i32::from(v.desc_0501[0]), wait_file(v.desc_0501[0]));
                f.names.extend(["Vi0501.dat", waits].map(String::from));
                stage(&mut f, 17);
                f.player(0, ck, v.desc_0501[1]);
                for slot in 1..4 {
                    f.player(slot, 4, v.desc_0501[3]);
                }
                f.anims(waits, ck, 3, 8);
                f.anims("Vi0501.dat", 4, 6, 11);
                for i in 4..7 {
                    f.copies
                        .push((usize::from(v.desc_0501[i]), usize::from(v.desc_0501[3])));
                }
            }
            0x15 => {
                let (ck, waits) = (i32::from(v.desc_0502[0]), wait_file(v.desc_0502[0]));
                f.names
                    .extend(["Vi0502.dat", waits, "IrAls.dat"].map(String::from));
                stage(&mut f, 17);
                f.player(0, ck, v.desc_0502[1]);
                f.player(1, 4, v.desc_0502[3]);
                f.anims(waits, ck, 3, 8);
                f.anims("Vi0502.dat", 4, 7, 14);
            }
            0x16 => {
                f.names.push("Vi0601.dat".into());
                f.songs.push(0x57);
                stage(&mut f, 7);
            }
            0x17 => {
                f.names.push("Vi0801.dat".into());
                f.songs.push(0x5B);
                stage(&mut f, 73);
            }
            s => super::code::unported_call(&format!("the cutscene scene {s:#x}'s files")),
        }
        Some(f)
    }
}

/// Reads a cutscene's files (`files`), its fighters' as a match's (`PlCo.dat`, each
/// character's data and costume), and each demo animation its fighters take, one archive
/// each, as `ftData_80085CD8` parses it from its copy.
pub fn load_scene_files(
    game: &mut super::state::Game,
    data: &mut melee_data::archive::Archives,
    files: &SceneFiles,
    load: &mut dyn FnMut(&str) -> super::vs_match::DiscFile,
    after: &mut dyn FnMut(&str) -> Vec<u8>,
) {
    use melee_data::archive::Archive;
    for &bgm in &files.songs {
        let file = load(&format!("audio/{}", crate::lb::audio::HPS_NAMES[bgm]));
        let devcom = &mut game.audio.devcom.files;
        if !devcom.iter().any(|f| f.0 == file.entrynum) {
            let id = data.add(Archive::raw(&file.bytes));
            devcom.push((file.entrynum, id));
        }
    }
    for name in &files.names {
        let loaded = &game.ft.data.files;
        if loaded.archives.iter().any(|a| &a.0 == name) {
            continue;
        }
        let file = load(name);
        let a = Archive::parse_in(&file.bytes, &after(name))
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let id = data.add(a);
        let loaded = &mut game.ft.data.files;
        loaded.archives.push((name.clone(), id));
        loaded
            .entries
            .push((name.clone(), file.entrynum, file.bytes.len() as u32));
    }
    super::vs_match::load_fighter_files(game, data, &files.players, load);
    for &(kind, costume) in &files.copies {
        use super::ft::kirby::init::{COPY_FILES, copy_costume_files};
        let mut names = Vec::new();
        if let Some((file, _)) = COPY_FILES[kind] {
            names.push(file.to_owned());
        }
        if let Some((file, _, _)) = copy_costume_files(kind, costume) {
            names.push(file);
        }
        for name in names {
            let loaded = &game.ft.data.files;
            if loaded.archives.iter().any(|a| a.0 == name) {
                continue;
            }
            let file = load(&name);
            let a = Archive::parse(&file.bytes).unwrap_or_else(|e| panic!("{name}: {e:?}"));
            let id = data.add(a);
            let loaded = &mut game.ft.data.files;
            loaded.archives.push((name.clone(), id));
            loaded
                .entries
                .push((name, file.entrynum, file.bytes.len() as u32));
        }
    }
    for (name, kind, idx, lo, hi) in &files.anims {
        let loaded = &game.ft.data.files;
        let archive = loaded.archive(name);
        let sym = super::ft::demo::demo_symbol(*kind, *idx).expect("a demo symbol");
        let base = data
            .get(archive)
            .public(&sym)
            .unwrap_or_else(|| panic!("{sym} isn't in {name}"));
        let t = super::ft::data::kind_tables(*kind);
        let dat = loaded.archive(t.dat_file.expect("a fighter kind without a data file"));
        let ft_data = super::ft::data::public(
            data,
            dat,
            t.data_symbol.expect("a fighter kind without data"),
        );
        let list = data
            .ptr(ft_data.at(0x14))
            .expect("a kind's demo animations");
        for i in *lo..=*hi {
            let e = list.at(0x18 * i as u32);
            let (x4, x8) = (data.u32(e.at(4)), data.u32(e.at(8)));
            let off = base.wrapping_add(x4);
            if x8 == 0 || game.ft.data.files.demo_anim(archive, off).is_some() {
                continue;
            }
            let bytes = &data.get(archive).data()[off as usize..(off + x8) as usize];
            let a = Archive::parse(bytes).unwrap_or_else(|e| panic!("{name}'s {sym}: {e:?}"));
            let id = data.add(a);
            let loaded = &mut game.ft.data.files;
            loaded.demo_anims.push((archive, off, id));
            loaded
                .archives
                .push((format!("{name}@{:#x}", off + 0x20), id));
        }
    }
}
