//! The single-player intros. Classic's (`gm_1832.c`, scene `GS_INTRO_EASY`) shows the stage's
//! number and the player against the stage's opponents before each stage: the player frozen
//! in a KO toward the screen and the opponents asleep, posed at the screen's model's joints
//! under two cameras, their names written in, the announcer calling the first opponent; a
//! bonus stage's intro shows its title instead. Adventure's (`gm_1879.c`, `GS_INTRO_NORMAL`)
//! flies a camera over the stage's map until Start.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::code::ParticleHooks;
use super::super::handles::CodeRef;
use super::super::render::{CAMERA_CALLBACK, FOG_CALLBACK, JOBJ_CALLBACK, LOBJ_CALLBACK};
use super::super::state::{Game, OBJ_CAMERA, OBJ_FOG, OBJ_JOBJ, OBJ_LIGHT, Obj, Running};
use super::super::vi::Loaded;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;

/// `GS_INTRO_NORMAL`, Adventure's intro, and `GS_INTRO_EASY`, Classic's.
pub const GS_INTRO_NORMAL: u8 = 0x0E;
pub const GS_INTRO_EASY: u8 = 0x20;

/// The scenes' entries and frame functions (`gmscdata.c`'s table).
pub const EASY_ON_ENTER: CodeRef = CodeRef(0x8018_6E30);
pub const EASY_ON_FRAME: CodeRef = CodeRef(0x8018_6DFC);
pub const NORMAL_ON_ENTER: CodeRef = CodeRef(0x8018_7F48);
pub const NORMAL_ON_FRAME: CodeRef = CodeRef(0x8018_838C);

/// Classic's intro's processes: the screen's model's (`fn_80184AB8`), the map's
/// (`fn_8018504C`), and the fighters' (`fn_801849E0` to `fn_80184A28` the player's side,
/// `fn_80184A4C` to `fn_80184A94` the opponents').
pub const EASY_MODEL_PROC: u32 = 0x8018_4AB8;
pub const EASY_MAP_PROC: u32 = 0x8018_504C;
pub const EASY_FIGHTER_PROCS: [u32; 6] = [
    0x8018_49E0,
    0x8018_4A04,
    0x8018_4A28,
    0x8018_4A4C,
    0x8018_4A70,
    0x8018_4A94,
];
/// The second camera's render callback (`fn_8018575C`; `fn_8018569C` for the horde stages').
pub const EASY_CAMERA_RENDER: u32 = 0x8018_575C;
pub const EASY_HORDE_CAMERA_RENDER: u32 = 0x8018_569C;

/// Adventure's intro's processes and render callback: the camera's (`fn_80187910`), the
/// stage's map (`fn_80187AB4`), the title (`fn_80187CF4`), and the models' (`fn_80187C9C`).
pub const NORMAL_CAMERA_PROC: u32 = 0x8018_7910;
pub const NORMAL_MAP_PROC: u32 = 0x8018_7AB4;
pub const NORMAL_TITLE_PROC: u32 = 0x8018_7CF4;
pub const NORMAL_MODEL_RENDER: u32 = 0x8018_7C9C;

/// `ChKind_None`, `CKind_GKoops`, `CKind_Koopa`, `CKind_MasterHand`.
const NONE: u8 = 0x21;
const CKIND_GKOOPS: u8 = 0x1D;
const CKIND_KOOPA: u8 = 5;
const CKIND_MASTERH: u8 = 0x1A;

/// `lbl_803B7C58`: the screen's archive by the bonus stages' game type (none, Target Test,
/// the race, Snag the Trophies).
const GAME_TYPE_ARCHIVES: [&str; 4] = ["IrAls", "IrEzTarg", "IrEzTuki", "IrEzFigG"];
/// `lbl_804D40B0`, the map's archive, and the scenes' symbol (`lbl_803D9444`).
const MAP_ARCHIVE: &str = "IrRdMap";
const SCENE_SYMBOL: &str = "ScItrAllstar_scene_data";
/// `GmIntEz.dat`'s table.
const TABLE_ARCHIVE: &str = "GmIntEz.dat";
const TABLE_SYMBOL: &str = "gmIntroEasyTable";

/// The intro's enter data and what it shows (`lbl_8047368C`, `gm_1832_8047368C_t`): the
/// 0x20 bytes of `ClassicModeEnterData` the entry copies over its first, then padding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct EasyData {
    /// `model_scale_kind`: 0 one opponent, 1 a team stage, 2 a giant's, 3 a bonus stage,
    /// 4 a horde.
    pub kind: i32,
    /// `game_type`: the bonus stage (1 Target Test, 2 the race, 3 Snag the Trophies).
    pub game_type: i32,
    /// `xEC`, `xED`: the player's port and name tag; `xEE`: the stage's number.
    pub slot: u8,
    pub nametag: u8,
    pub stage: u8,
    /// `xEF`, `xF0`: the player's side's count and the opponents'.
    pub allies: u8,
    pub enemies: u8,
    /// `xF1`, `xF4`, `xF7`, `xFA`, `xFD`, `x100`: the sides' characters, costumes and metal
    /// flags.
    pub ally_kinds: [u8; 3],
    pub enemy_kinds: [u8; 3],
    pub ally_colors: [u8; 3],
    pub enemy_colors: [u8; 3],
    pub ally_metal: [u8; 3],
    pub enemy_metal: [u8; 3],
    /// The enter data's last byte and the four after it.
    pub x1f: u8,
    pub pad: [u8; 4],
}

impl EasyData {
    /// The struct as memory holds it.
    pub fn bytes(&self) -> [u8; 0x24] {
        let mut b = [0u8; 0x24];
        b[0..4].copy_from_slice(&self.kind.to_be_bytes());
        b[4..8].copy_from_slice(&self.game_type.to_be_bytes());
        b[8] = self.slot;
        b[9] = self.nametag;
        b[0xA] = self.stage;
        b[0xB] = self.allies;
        b[0xC] = self.enemies;
        b[0xD..0x10].copy_from_slice(&self.ally_kinds);
        b[0x10..0x13].copy_from_slice(&self.enemy_kinds);
        b[0x13..0x16].copy_from_slice(&self.ally_colors);
        b[0x16..0x19].copy_from_slice(&self.enemy_colors);
        b[0x19..0x1C].copy_from_slice(&self.ally_metal);
        b[0x1C..0x1F].copy_from_slice(&self.enemy_metal);
        b[0x1F] = self.x1f;
        b[0x20..].copy_from_slice(&self.pad);
        b
    }

    /// The enter data's 0x20 bytes copied over the first (`*(ClassicModeEnterData*)`).
    fn copy_enter(&mut self, e: &[u8; 0x20]) {
        let w = |i: usize| i32::from_be_bytes([e[i], e[i + 1], e[i + 2], e[i + 3]]);
        self.kind = w(0);
        self.game_type = w(4);
        self.slot = e[8];
        self.nametag = e[9];
        self.stage = e[0xA];
        self.allies = e[0xB];
        self.enemies = e[0xC];
        self.ally_kinds.copy_from_slice(&e[0xD..0x10]);
        self.enemy_kinds.copy_from_slice(&e[0x10..0x13]);
        self.ally_colors.copy_from_slice(&e[0x13..0x16]);
        self.enemy_colors.copy_from_slice(&e[0x16..0x19]);
        self.ally_metal.copy_from_slice(&e[0x19..0x1C]);
        self.enemy_metal.copy_from_slice(&e[0x1C..0x1F]);
        self.x1f = e[0x1F];
    }
}

/// Classic's intro's statics (`gm_1832.c`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct IntroEasy {
    /// `lbl_804735A8.x0`: the scene's end asked for.
    pub done: i32,
    /// `x4[0]` to `x4[5]`: the screen's model's joints 4, 5, 0x3A and 0x3B (the player's and
    /// the opponents' places, two teammates' marks), and the map's 0xE and 1.
    pub joints: [Option<JObjId>; 6],
    /// `x4[6]` to `x4[12]`: the bonus stage's title, the player's side's names and the
    /// opponents'.
    pub texts: [Option<TextId>; 7],
    /// `x38`: the screen's tick; `x3A`, `x3C`: the map's frames.
    pub tick: u16,
    pub map_frame: u16,
    pub map_frame2: u16,
    pub x3e: u16,
    /// `lbl_8047368C`.
    pub data: EasyData,
    /// `lbl_804D65F4`, `lbl_804D65F8`: the screen's archive and the map's.
    pub archive: Loaded,
    pub map_archive: Loaded,
    /// `lbl_804D65FC`, `lbl_804D6600`: their scenes; `lbl_804D6604`: `GmIntEz.dat`'s table.
    pub scene: Option<DataRef>,
    pub map_scene: Option<DataRef>,
    pub table: Option<DataRef>,
    /// `lbl_804D6608`: the announcer's call's sound.
    pub call: i32,
}

/// Adventure's intro's statics (`gm_1879.c`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct IntroNormal {
    /// `lbl_804736C0.x0`, `x4`: the archive's scene and the stage's camera (`mcNN`, a
    /// `SceneCameraDesc`); `x8`: that camera's GObj.
    pub scene: Option<DataRef>,
    pub camera: Option<DataRef>,
    pub gobj: Option<GObjId>,
    /// `x34`: set to 2400 once the map's first animation ends, never read.
    pub x34: u16,
    /// `x36`'s bits: the stage's number, the scene's end asked for, the flash started, Start
    /// pressed.
    pub stage: u8,
    pub done: bool,
    pub flash: bool,
    pub active: bool,
    /// `x37`'s bits: the camera's stretch (of eight, 300 frames each), the map's animation,
    /// the title's.
    pub stretch: u8,
    pub map_anim: u8,
    pub title_anim: u8,
    /// `x38`: the player's port.
    pub slot: u8,
    /// `lbl_804D6620`: `IrNml`'s archive.
    pub archive: Loaded,
}

/// The stage's camera's frame in Adventure's intro, `gm_801883C0`: its eye's animation's, else
/// its interest's, else 0, and 0 without the camera (`lbl_804736C0.x8`).
#[doc(alias = "gm_801883C0")]
pub fn stage_camera_frame(game: &Game) -> f32 {
    let Some(gobj) = game.intros.normal.gobj else {
        return 0.0;
    };
    let Some(Obj::CObj(c)) = game.world.gobj(gobj).obj else {
        panic!("the intro's camera holds no CObj");
    };
    let objs = &game.objs;
    let cobj = &objs.cobjs[c];
    let eye = cobj.eyepos.expect("a camera without an eye");
    let interest = cobj.interest.expect("a camera without an interest");
    [eye, interest]
        .into_iter()
        .find_map(|w| objs.wobjs[w.0].aobj)
        .map_or(0.0, |a| objs.aobjs[a].curr_frame)
}

impl IntroNormal {
    /// `x36` and `x37` as memory holds their bits.
    pub fn bits(&self) -> [u8; 2] {
        [
            (self.stage << 3)
                | (u8::from(self.done) << 2)
                | (u8::from(self.flash) << 1)
                | u8::from(self.active),
            (self.stretch << 4) | (self.map_anim << 2) | self.title_anim,
        ]
    }
}

/// The single-player intros' statics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Intros {
    pub easy: IntroEasy,
    pub normal: IntroNormal,
}

/// Whether `scene` is an intro this module ports.
pub fn is_intro(scene: u8) -> bool {
    scene == GS_INTRO_EASY || scene == GS_INTRO_NORMAL
}

/// `gm_1832_NormalArchiveNames`, `gm_1832_NormalBgmIds`, `gm_1832_NormalAudioMasks`: each
/// Adventure stage's camera's symbol in `IrNml`, its song and its sound banks.
const NORMAL_CAMERAS: [&str; 12] = [
    "mc01", "mc02", "mc03", "mc04", "mc05", "mc06", "mc07", "mc08", "mc09", "mc10", "mc11", "mc12",
];
const NORMAL_SONGS: [i32; 12] = [
    0x1C, 0x1C, 0x1C, 0x1C, 0x1C, 0x1C, 0x1C, 0x1C, 0x1C, 0x14, 0x14, 0x1C,
];
const NORMAL_BANKS: [u64; 12] = [
    0x0002_0000_1000_0000,
    0x0000_0020_0000_0100,
    0x0002_0000_0001_0000,
    0x0000_0100_0400_0000,
    0x0000_0802_0000_4000,
    0x0000_0008_0000_0C00,
    0x0001_0000_0100_0000,
    0x0002_0000_0000_0040,
    0x0000_0080_0020_0000,
    0x0000_0000_0000_2000,
    0x0000_0000_0006_0000,
    0x0020_0000_0000_9000,
];
/// `IrNml`'s archive and its scene's symbol (`gm_1832_NormalSceneDataName`).
const NORMAL_ARCHIVE: &str = "IrNml";
const NORMAL_SCENE_SYMBOL: &str = "ScItrNormal_scene_data";
/// The stage's camera's render callback (`Camera_800304E0`).
pub const NORMAL_STAGE_CAMERA_RENDER: u32 = 0x8003_04E0;

/// Offsets in `gmIntroEasyTable` (`lbl_804D6604_t`).
mod table {
    /// `x00[2]`, `x18[3]`, `x3C[4]`: each count's x offsets, y offsets and scales (three
    /// floats each); `x6C[28]`: each character's place (x, y, a scale vector; 0x1C each);
    /// `x37C[25]`: a giant's (x, y, scale; 0x14 each); `x57C[3]`: the names' rows (x, y and
    /// the two scales, three floats each; 0x30 each); `x630`, `x654`, `x678`, `x6A8`: the
    /// giants' stage's own.
    pub const X00: u32 = 0x00;
    pub const X18: u32 = 0x18;
    pub const X3C: u32 = 0x3C;
    pub const X6C: u32 = 0x6C;
    pub const X37C: u32 = 0x37C;
    pub const X57C: u32 = 0x57C;
    pub const X630: u32 = 0x630;
    pub const X654: u32 = 0x654;
    pub const X678: u32 = 0x678;
    pub const X6A8: u32 = 0x6A8;
}

/// `HSD_GOBJ_CLASS_UI`, `HSD_GOBJ_CLASS_CAMERA`, `HSD_GOBJ_CLASS_LIGHT`.
const CLASS_UI: u16 = 0xE;
const CLASS_CAMERA: u16 = 0x13;
const CLASS_LIGHT: u16 = 0xB;

impl Running<'_> {
    fn easy_table_f32(&self, off: u32) -> f32 {
        let t = self.game.intros.easy.table.expect("the intro's table");
        self.data.f32(t.at(off))
    }

    /// A slot's values row `row` of a three-float table at `base`.
    fn easy_vals(&self, base: u32, row: i32, i: usize) -> f32 {
        self.easy_table_f32(base.wrapping_add((row * 0xC) as u32) + 4 * i as u32)
    }

    /// A character's place (`ClassicCharLayout`) field at `field` in a table at `base`.
    fn easy_layout(&self, base: u32, ckind: u8, field: u32) -> f32 {
        self.easy_table_f32(base + 0x1C * u32::from(ckind) + field)
    }

    /// The names' row for a count (`x57C[count]`): its float `field` (0 x, 0xC y, 0x18 and
    /// 0x24 the scales) of slot `i`.
    fn easy_row(&self, count: u8, field: u32, i: usize) -> f32 {
        self.easy_table_f32(table::X57C + 0x30 * u32::from(count) + field + 4 * i as u32)
    }

    /// The scene's enter data, Classic's (`gmClassicIntroDataBuffer`) or the debug mode's.
    fn intro_easy_enter(&self) -> [u8; 0x20] {
        if self.game.game_mode == super::one_player::GM_CLASSIC {
            return self.game.one_player.classic_intro;
        }
        assert!(
            self.debug_mode_scene(),
            "Classic's intro in game mode {:#x}",
            self.game.game_mode
        );
        self.game.endings.intro_easy
    }

    /// `gm_Scene_IntroEasy_OnEnter`: nothing (the scene ending at once) when Master Hand is
    /// either of the first two opponents; else the screen's tick and the map's frames reset,
    /// the enter data copied, the scene (`fn_80186634`), the rumble on the player's port and
    /// the announcer's bank.
    #[doc(alias = "gm_Scene_IntroEasy_OnEnter")]
    pub fn intro_easy_on_enter(&mut self) {
        // The scene takes the files the preload cache holds (`lbArchive_80017040`): the
        // common effects and items, the fighters'.
        self.game.ft.data.files.from_cache = true;
        let e = self.intro_easy_enter();
        let s = &mut self.game.intros.easy;
        if e[0x10] == CKIND_MASTERH || e[0x11] == CKIND_MASTERH {
            s.done = 1;
            return;
        }
        s.done = 0;
        s.tick = 0;
        s.map_frame = 0;
        s.map_frame2 = 0;
        s.data.copy_enter(&e);
        self.intro_easy_setup();
        self.game.intros.easy.call = -1;
        let d = self.game.intros.easy.data;
        // The pad alarm's firings during the loads ran before the rumble starts.
        self.alarm_rumble_catch_up();
        self.tou_rumble(d.slot as i8 as usize, d.nametag, 0xB, 0x2D);
        self.load_announcer_bank();
    }

    /// `gm_Scene_IntroEasy_OnFrame`: once the screen asked to end, the music stopped and the
    /// scene's end.
    #[doc(alias = "gm_Scene_IntroEasy_OnFrame")]
    pub fn intro_easy_on_frame(&mut self) {
        if self.game.intros.easy.done != 0 {
            self.lb_audio_music_stop();
            self.game.end_scene();
        }
    }

    /// `fn_80186634`: `GmIntEz.dat`'s table, the camera's, dynamics', collisions' and dummy
    /// stage's setups, the items' and effects', the screen's archive (by the bonus stage) and
    /// the map's, the light, then by the kind the cameras, the screen's model, the map, the
    /// fog, and the names with the fighters (or a bonus stage's title), and the intro's song.
    #[doc(alias = "fn_80186634")]
    fn intro_easy_setup(&mut self) {
        let (ta, _, _) = self.load_archive_blocks(TABLE_ARCHIVE);
        let off = self
            .data
            .get(ta)
            .public(TABLE_SYMBOL)
            .unwrap_or_else(|| panic!("{TABLE_SYMBOL} isn't in {TABLE_ARCHIVE}"));
        self.game.intros.easy.table = Some(DataRef {
            archive: ta,
            offset: off,
        });
        super::super::camera::init(&mut self.game.camera, 0xC);
        self.game.alloc_dynamics(self.data);
        self.game.coll.reset();
        super::super::gr::setup::prepare(self.game);
        super::super::gr::setup::select(self, 0, None);
        let kept = self.take_cached_named("ItCo.usd");
        let it_co = self.game.ft.data.files.archive("ItCo.usd");
        super::super::it::item::load_common_data(self.game, self.data, it_co, kept);
        super::super::it::item::init_globals(self.game, self.data);
        let grkind = self.game.stage_info.grkind;
        super::super::gr::setup::setup_at(self, grkind, 0x28);
        super::super::ef::eflib::init(self);
        super::super::ef::load_sync(self, 0);
        let game_type = self.game.intros.easy.data.game_type;
        let name = self.intro_file_name(GAME_TYPE_ARCHIVES[game_type as usize]);
        let (archive, scene) = self.intro_scene_archive(&name);
        let map_name = self.intro_file_name(MAP_ARCHIVE);
        let (map_archive, map_scene) = self.intro_scene_archive(&map_name);
        let s = &mut self.game.intros.easy;
        s.archive = archive;
        s.scene = Some(scene);
        s.map_archive = map_archive;
        s.map_scene = Some(map_scene);
        self.intro_easy_light(scene);
        let kind = self.game.intros.easy.data.kind;
        if kind != 3 {
            self.game.intros.easy.data.game_type = 0;
        }
        if (0..=4).contains(&kind) {
            self.intro_easy_cameras(scene, map_scene, kind == 4);
            self.intro_easy_model(scene);
            self.intro_easy_map(map_scene);
            self.intro_easy_fog(scene);
        }
        match kind {
            3 => self.intro_easy_title(),
            0..=2 => {
                self.intro_easy_names();
                self.intro_easy_demo_files();
                let next = self.intro_easy_allies();
                self.intro_easy_enemies(next);
            }
            4 => super::super::code::unported_call(
                "Classic's horde stage's intro (fn_80185A0C, its image splash)",
            ),
            _ => {}
        }
        self.lb_play_music(0x2D);
    }

    /// A name `lbFileGetFullName` completes with the saved language's extension.
    fn intro_file_name(&self, base: &str) -> String {
        format!("{base}.{}", if self.files_us() { "usd" } else { "dat" })
    }

    /// An archive into the scene's heap (`lbArchive_80016DBC`) and its scene.
    fn intro_scene_archive(&mut self, name: &str) -> (Loaded, DataRef) {
        let (a, file, block) = self.load_archive_blocks(name);
        let offset = self
            .data
            .get(a)
            .public(SCENE_SYMBOL)
            .unwrap_or_else(|| panic!("{SCENE_SYMBOL} isn't in {name}"));
        (
            Loaded {
                archive: Some(a),
                file,
                block,
            },
            DataRef { archive: a, offset },
        )
    }

    /// `gm_80186634_SetupLight`: the screen's scene's lights on a light GObj.
    fn intro_easy_light(&mut self, scene: DataRef) {
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_LIGHT, 3, 0);
        let lights = data.ptr(scene.at(8)).expect("the scene's lights");
        let l = super::super::gr::setup::load_light_list(g, data, lights);
        g.world
            .set_obj(gobj, OBJ_LIGHT, Obj::LObj(l.expect("a light")));
        g.world.setup_gx_link(gobj, CodeRef(LOBJ_CALLBACK), 0, 0);
    }

    /// `gm_80186634_SetupCamera`: the screen's scene's camera drawing render links 0 and 11,
    /// then the map's, whose callback draws the fighters (`fn_8018575C`, the horde stage's
    /// `fn_8018569C`).
    fn intro_easy_cameras(&mut self, scene: DataRef, map: DataRef, horde: bool) {
        let data = self.data;
        let camera = |s: DataRef| {
            let cams = data.ptr(s.at(4)).expect("the scene's cameras");
            data.ptr(cams)
        };
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_CAMERA, 0x14, 0);
        let c = crate::hsd::cobj::load_desc(&mut g.objs, data, camera(scene)).expect("a camera");
        let c2 = crate::hsd::cobj::load_desc(&mut g.objs, data, camera(map)).expect("a camera");
        g.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(gobj, CodeRef(CAMERA_CALLBACK), 8, false);
        g.world.gobj_mut(gobj).gxlink_prios = 0x801;
        let gobj2 = g.world.create(CLASS_CAMERA, 0x15, 0);
        g.world.set_obj(gobj2, OBJ_CAMERA, Obj::CObj(c2));
        let render = if horde {
            EASY_HORDE_CAMERA_RENDER
        } else {
            EASY_CAMERA_RENDER
        };
        g.world.setup_gx_link_max(gobj2, CodeRef(render), 8, false);
    }

    /// `fn_80186400`: the screen's model with its process (`fn_80184AB8`), its first
    /// animations started; but on a bonus stage, its joints 4 and 5 (the sides' places) and
    /// 0x3A and 0x3B (two teammates' marks, put out of sight).
    #[doc(alias = "fn_80186400")]
    fn intro_easy_model(&mut self, scene: DataRef) {
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_UI, 0xF, 0);
        let models = data.ptr(scene).expect("the scene's models");
        let m = data.ptr(models).expect("the screen's model");
        let j = jobj::load_joint(&mut g.objs, data, data.ptr(m)).expect("the screen's model");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 0xB, 0);
        g.world.setup_proc(gobj, CodeRef(EASY_MODEL_PROC), 0);
        super::vs::add_model_anim(&mut g.objs, data, Some(j), m, 0);
        jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
        self.anim_all(j);
        if self.game.intros.easy.data.kind != 3 {
            let jobjs = &mut self.game.objs.jobjs;
            for (k, index) in [(0, 4), (1, 5), (2, 0x3A), (3, 0x3B)] {
                self.game.intros.easy.joints[k] =
                    super::pause::tree_nodes(jobjs, Some(j), &[index])[0];
            }
            for k in [2, 3] {
                let n = self.game.intros.easy.joints[k].expect("a teammate's mark");
                jobj::set_srt(jobjs, n, |o| o.translate.z = 10000.0);
            }
        }
    }

    /// `gm_80186634_SetupModel`: the map with its process (`fn_8018504C`), its animation at
    /// the stage's 50 frames, and its joints 0xE and 1.
    fn intro_easy_map(&mut self, map: DataRef) {
        let data = self.data;
        let stage = self.game.intros.easy.data.stage;
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_UI, 0xF, 0);
        let models = data.ptr(map).expect("the map's models");
        let m = data.ptr(models).expect("the map's model");
        let j = jobj::load_joint(&mut g.objs, data, data.ptr(m)).expect("the map");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 0xC, 0);
        g.world.setup_proc(gobj, CodeRef(EASY_MAP_PROC), 0x11);
        super::vs::add_model_anim(&mut g.objs, data, Some(j), m, 0);
        let frame = (i32::from(stage) - 1).wrapping_mul(0x32);
        jobj::req_anim_all(&mut g.objs, Some(j), mem(fp::frsp(f64::from(frame))));
        self.anim_all(j);
        let jobjs = &self.game.objs.jobjs;
        let s = &mut self.game.intros.easy;
        s.joints[4] = super::pause::tree_nodes(jobjs, Some(j), &[0xE])[0];
        s.joints[5] = super::pause::tree_nodes(jobjs, Some(j), &[1])[0];
    }

    /// `gm_80186634_SetupFog`: the screen's scene's fog.
    fn intro_easy_fog(&mut self, scene: DataRef) {
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_UI, 0xF, 0);
        let fogs = data.ptr(scene.at(0xC)).expect("the scene's fogs");
        let desc = data.ptr(fogs).expect("the scene's fog");
        let f = crate::hsd::fog::load_desc(&mut g.objs.fogs, data, desc);
        g.world.set_obj(gobj, OBJ_FOG, Obj::Fog(f));
        g.world.setup_gx_link(gobj, CodeRef(FOG_CALLBACK), 0xB, 0);
    }

    /// `fn_80186080`: a bonus stage's title, `SdIntro.dat`'s text for its game type and the
    /// saved language.
    // Ports fn_80186080; aliased once a check reaches it (a bonus stage's intro).
    fn intro_easy_title(&mut self) {
        let canvas = self.add_canvas(0, None, 9, 0x14, 0, 0xE, 0, 0x12);
        self.load_font_named(0, "SdIntro.dat", "SIS_IntroData");
        let t = self.create_text(0, canvas, crate::vec::Vec3::ZERO, 640.0, 480.0);
        self.game.intros.easy.texts[0] = Some(t);
        let us = self.game.game_data.saved_language() == super::main_lib::LANG_US;
        let index = match self.game.intros.easy.data.game_type {
            1 => Some(if us { 5 } else { 2 }),
            3 => Some(if us { 6 } else { 3 }),
            2 => Some(if us { 7 } else { 4 }),
            _ => None,
        };
        if let Some(i) = index {
            self.game.sis.reset_text(self.data, t, i);
        }
    }

    /// `fn_801861B8`: the names' texts, blank, at their rows for the sides' counts (the
    /// opponents' 340 to the right, their first scale too; a horde's one, at the single
    /// row).
    #[doc(alias = "fn_801861B8")]
    fn intro_easy_names(&mut self) {
        self.add_canvas(0, None, 9, 0x14, 0, 0xE, 0, 0x12);
        self.load_font_named(0, "SdIntro.dat", "SIS_IntroData");
        let d = self.game.intros.easy.data;
        for i in 0..usize::from(d.allies) {
            let t = self.intro_easy_name_text(7 + i);
            let (x, y) = (
                self.easy_row(d.allies, 0, i),
                self.easy_row(d.allies, 0xC, i),
            );
            let (sx, sy) = (
                self.easy_row(d.allies, 0x18, i),
                self.easy_row(d.allies, 0x24, i),
            );
            self.game
                .sis
                .add_string(t, f64::from(x), f64::from(y), Some(b" \0"));
            self.game
                .sis
                .set_scale(self.data, t, 0, f64::from(sx), f64::from(sy));
        }
        for i in 0..usize::from(d.enemies) {
            let t = self.intro_easy_name_text(10 + i);
            let (row, k) = if d.kind == 4 { (1, 0) } else { (d.enemies, i) };
            let x = fp::fadds(340.0, reg(self.easy_row(row, 0, k)));
            let y = self.easy_row(row, 0xC, k);
            let sx = fp::fadds(340.0, reg(self.easy_row(row, 0x18, k)));
            let sy = self.easy_row(row, 0x24, k);
            self.game.sis.add_string(t, x, f64::from(y), Some(b" \0"));
            self.game.sis.set_scale(self.data, t, 0, sx, f64::from(sy));
            if d.kind == 4 {
                return;
            }
        }
    }

    /// A name's text (`HSD_SisLib_803A6754`): kerned, at size 1, centered; kept as
    /// `x4[slot]`.
    fn intro_easy_name_text(&mut self, slot: usize) -> TextId {
        let t = self.create_text_buffer(0, 0);
        self.game.intros.easy.texts[slot - 6] = Some(t);
        let x = self.game.sis.text_mut(t);
        x.default_kerning = 1;
        x.font_size = [1.0, 1.0];
        x.default_alignment = 1;
        t
    }

    /// `fn_80185D64`: the fighters' setup for demos, the players reset, and each side's
    /// characters' intro animations from the screen's archive (a Giga Bowser opponent taken
    /// as Bowser).
    #[doc(alias = "fn_80185D64")]
    fn intro_easy_demo_files(&mut self) {
        self.fighter_demo_init();
        self.game.init_all_players();
        let a = self.game.intros.easy.archive;
        let archive = a.archive.expect("the screen's archive");
        let d = self.game.intros.easy.data;
        for &c in &d.ally_kinds[..usize::from(d.allies)] {
            if c != NONE {
                self.player_set_demo_archive_idx(i32::from(c), archive, a.file, 1);
            }
        }
        for i in 0..usize::from(d.enemies) {
            let s = &mut self.game.intros.easy.data;
            if s.enemy_kinds[i] == CKIND_GKOOPS {
                s.enemy_kinds[i] = CKIND_KOOPA;
            }
            let c = s.enemy_kinds[i];
            if c != NONE {
                self.player_set_demo_archive_idx(i32::from(c), archive, a.file, 1);
            }
        }
    }

    /// A demo player of the intro's: its character and costume, port 0, the demo slot type,
    /// facing 0, size 1, metal or not, its fighters in pose `pose` and their process.
    fn intro_easy_player(&mut self, slot: i32, ckind: u8, costume: u8, metal: u8, pose: i32) {
        self.game.reinit_player(slot);
        self.game.set_player_ckind(slot, i32::from(ckind));
        self.game.set_player_costume(slot, i32::from(costume));
        self.game.set_player_port(slot, 0);
        self.game
            .set_player_pkind(slot, super::super::pl::player::PKIND_DEMO);
        self.game.set_player_facing(slot, 0.0);
        self.game.set_player_model_scale(slot, 1.0);
        self.game.set_player_is_metal(slot, metal);
        self.player_create_demo(slot, pose);
    }

    /// `fn_80185E34`: the player's side from slot 0, each frozen in a KO toward the screen
    /// (pose 5) with its place's process; the next slot.
    #[doc(alias = "fn_80185E34")]
    fn intro_easy_allies(&mut self) -> i32 {
        let d = self.game.intros.easy.data;
        let mut slot = 0;
        for (i, &proc) in EASY_FIGHTER_PROCS[..usize::from(d.allies)]
            .iter()
            .enumerate()
        {
            if d.ally_kinds[i] == NONE {
                continue;
            }
            self.intro_easy_player(slot, d.ally_kinds[i], d.ally_colors[i], d.ally_metal[i], 5);
            let g = self.game.player_entity(slot).expect("a demo fighter");
            self.game.world.setup_proc(g, CodeRef(proc), 0x16);
            slot += 1;
        }
        slot
    }

    /// `fn_80185F5C`: the opponents from slot `slot`, each asleep (pose 6) with its place's
    /// process. Each one's test is of the player's side's character in its place, not its
    /// own ([Melee's bugs](../../../../../docs/melee-bugs.md)).
    #[doc(alias = "fn_80185F5C")]
    fn intro_easy_enemies(&mut self, mut slot: i32) {
        let d = self.game.intros.easy.data;
        for i in 0..usize::from(d.enemies) {
            if d.ally_kinds[i] == NONE {
                continue;
            }
            self.intro_easy_player(
                slot,
                d.enemy_kinds[i],
                d.enemy_colors[i],
                d.enemy_metal[i],
                6,
            );
            let g = self.game.player_entity(slot).expect("a demo fighter");
            self.game
                .world
                .setup_proc(g, CodeRef(EASY_FIGHTER_PROCS[i + 3]), 0x16);
            slot += 1;
        }
    }

    /// The fighter of `gobj`'s root joint, and its slot's other fighter's (Nana's) given the
    /// same place and size, as each place's process ends.
    fn intro_easy_follow(&mut self, gobj: GObjId, root: JObjId) {
        for i in 0..6 {
            if self.game.player_entity(i) != Some(gobj) {
                continue;
            }
            if let Some(e2) = self.game.player_entity_at(i, 1) {
                let j2 = self.root_jobj(e2);
                let jobjs = &mut self.game.objs.jobjs;
                let (t, s) = (jobjs[root].translate, jobjs[root].scale);
                jobj::set_srt(jobjs, j2, |o| o.translate = t);
                jobj::set_srt(jobjs, j2, |o| o.scale = s);
            }
        }
    }

    /// `fn_8018325C` (through `fn_801849E0`, `fn_80184A04`, `fn_80184A28`): the player's
    /// side's fighter `i` at the screen's joint 4, back 10 and 30 for the second and third
    /// (whose teammates' marks come in at ticks 81 and 91), moved and scaled by its
    /// character's place and the side's count (the giant stage's own tables).
    #[doc(alias = "fn_8018325C")]
    #[doc(alias = "fn_801849E0")]
    // Ports fn_80184A04 and fn_80184A28 (the second and third places); aliased once a check
    // reaches them.
    pub fn intro_easy_ally_proc(&mut self, gobj: GObjId, i: usize) {
        let s = self.game.intros.easy;
        let root = self.root_jobj(gobj);
        let src = s.joints[0].expect("the player's place");
        let jobjs = &mut self.game.objs.jobjs;
        let t = jobjs[src].translate;
        jobj::set_srt(jobjs, root, |o| o.translate = t);
        let mark = |r: &mut Self, k: usize, x: f32, y: f32, sc: f32| {
            let n = r.game.intros.easy.joints[k].expect("a teammate's mark");
            let jobjs = &mut r.game.objs.jobjs;
            jobj::set_srt(jobjs, n, |o| o.translate.x = x);
            jobj::set_srt(jobjs, n, |o| o.translate.y = y);
            jobj::set_srt(jobjs, n, |o| o.translate.z = 5.0);
            jobj::set_srt(jobjs, n, |o| o.scale.x = sc);
            jobj::set_srt(jobjs, n, |o| o.scale.y = sc);
        };
        match i {
            1 => {
                add_z(&mut self.game.objs.jobjs, root, -10.0);
                if s.tick > 0x50 {
                    mark(self, 2, -1.0, 0.0, 2.8);
                }
            }
            2 => {
                add_z(&mut self.game.objs.jobjs, root, -30.0);
                if s.tick > 0x5A {
                    mark(self, 3, -0.5, 3.0, 2.2);
                }
            }
            _ => {}
        }
        let d = s.data;
        let ck = d.ally_kinds[i];
        let count = i32::from(d.allies);
        let (layout, xs, ys, scales, xrow) = if d.kind == 2 {
            (table::X6A8, table::X630, table::X654, table::X678, count)
        } else {
            (table::X6C, table::X00, table::X18, table::X3C, count - 1)
        };
        let x = fp::fsubs(
            reg(self.easy_layout(layout, ck, 0)),
            reg(self.easy_vals(xs, xrow, i)),
        );
        let y = fp::fadds(
            reg(self.easy_layout(layout, ck, 4)),
            reg(self.easy_vals(ys, count, i)),
        );
        let sc = reg(self.easy_vals(scales, count, i));
        let sv = [
            mem(fp::fmuls(reg(self.easy_layout(layout, ck, 8)), sc)),
            mem(fp::fmuls(reg(self.easy_layout(layout, ck, 0xC)), sc)),
            mem(fp::fmuls(reg(self.easy_layout(layout, ck, 0x10)), sc)),
        ];
        self.intro_easy_place(root, x, y, sv);
        self.intro_easy_follow(gobj, root);
    }

    /// `fn_80184138` (through `fn_80184A4C`, `fn_80184A70`, `fn_80184A94`): opponent `i` at
    /// the screen's joint 5, the second and third 10 and 30 forward, moved by its character's
    /// place mirrored and the side's count, and scaled (on the giant stage by the giant's
    /// place, which moves it too).
    #[doc(alias = "fn_80184138")]
    #[doc(alias = "fn_80184A4C")]
    // Ports fn_80184A70 and fn_80184A94 (the second and third places); aliased once a check
    // reaches them.
    pub fn intro_easy_enemy_proc(&mut self, gobj: GObjId, i: usize) {
        let s = self.game.intros.easy;
        let root = self.root_jobj(gobj);
        let src = s.joints[1].expect("the opponents' place");
        let jobjs = &mut self.game.objs.jobjs;
        let t = jobjs[src].translate;
        jobj::set_srt(jobjs, root, |o| o.translate = t);
        let d = s.data;
        let ck = d.enemy_kinds[i];
        let giant = d.kind == 2;
        let giant_f =
            |r: &Self, field: u32| r.easy_table_f32(table::X37C + 0x14 * u32::from(ck) + field);
        let scl = if giant { giant_f(self, 8) } else { 1.0 };
        let xoff = if giant { giant_f(self, 0) } else { 0.0 };
        let yoff = if giant { giant_f(self, 4) } else { 0.0 };
        if i != 0 {
            let z = mem(fp::frsp(f64::from(-(i as i32) * 0x14 + 0xA)));
            add_z(&mut self.game.objs.jobjs, root, z);
        }
        let count = i32::from(d.enemies);
        let x = fp::fadds(
            reg(xoff),
            fp::fadds(
                fp::fneg(reg(self.easy_layout(table::X6C, ck, 0))),
                reg(self.easy_vals(table::X00, count - 1, i)),
            ),
        );
        let y = fp::fadds(
            reg(yoff),
            fp::fadds(
                reg(self.easy_layout(table::X6C, ck, 4)),
                reg(self.easy_vals(table::X18, count, i)),
            ),
        );
        let sc = if giant {
            reg(scl)
        } else {
            reg(self.easy_vals(table::X3C, count, i))
        };
        let sv = [
            mem(fp::fmuls(reg(self.easy_layout(table::X6C, ck, 8)), sc)),
            mem(fp::fmuls(reg(self.easy_layout(table::X6C, ck, 0xC)), sc)),
            mem(fp::fmuls(reg(self.easy_layout(table::X6C, ck, 0x10)), sc)),
        ];
        self.intro_easy_place(root, x, y, sv);
        self.intro_easy_follow(gobj, root);
    }

    /// A fighter's root moved by `x` and `y` (`HSD_JObjAddTranslationX`, `Y`) and scaled.
    fn intro_easy_place(&mut self, root: JObjId, x: f64, y: f64, scale: [f32; 3]) {
        let jobjs = &mut self.game.objs.jobjs;
        let tx = jobjs[root].translate.x;
        jobj::set_srt(jobjs, root, |o| o.translate.x = mem(fp::fadds(reg(tx), x)));
        let ty = jobjs[root].translate.y;
        jobj::set_srt(jobjs, root, |o| o.translate.y = mem(fp::fadds(reg(ty), y)));
        jobj::set_srt(jobjs, root, |o| o.scale.x = scale[0]);
        jobj::set_srt(jobjs, root, |o| o.scale.y = scale[1]);
        jobj::set_srt(jobjs, root, |o| o.scale.z = scale[2]);
    }

    /// `fn_80184AB8`, the screen's model's process: its animations; its tick counted to 140,
    /// then the scene's end asked for. But on a bonus stage: the stage's number's two
    /// digits (textures 0x12 and 0x13, held); a sound at tick 10; at 70 the announcer's
    /// call (by the kind, a metal opponent's own); at 100 it stopped and the first opponent's
    /// name called; at 5 the player's side's names (the player's name tag's instead); at 6
    /// the opponents' (a horde's one with its count).
    #[doc(alias = "fn_80184AB8")]
    pub fn intro_easy_model_proc(&mut self, gobj: GObjId) {
        let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj else {
            panic!("the intro's model holds no JObj");
        };
        self.anim_all(j);
        let s = &mut self.game.intros.easy;
        if s.tick < 0x8C {
            s.tick += 1;
        } else {
            s.done = 1;
            return;
        }
        let d = s.data;
        if d.kind == 3 {
            return;
        }
        let digits = if d.stage < 10 {
            [f32::from(d.stage % 10), 10.0]
        } else {
            [f32::from(d.stage / 10), f32::from(d.stage % 10)]
        };
        for (index, frame) in [(0x12, digits[0]), (0x13, digits[1])] {
            let objs = &mut self.game.objs;
            let n = super::pause::tree_nodes(&objs.jobjs, Some(j), &[index])[0];
            let t = digit_tobj(objs, n.expect("a digit's joint"));
            crate::hsd::tobj::req_anim_all(objs, t, frame);
            let a = t.and_then(|t| objs.tobjs[t].aobj);
            crate::hsd::aobj::set_rate(&mut objs.aobjs, a, 0.0);
        }
        match self.game.intros.easy.tick {
            0xA => {
                self.sfx_play(0x9C4A, 0x7F, 0x40);
            }
            0x46 => {
                let id = match d.kind {
                    4 | 1 => Some(0x7C863),
                    2 => Some(0x7C837),
                    _ if d.enemy_metal[0] != 0 => Some(0x7C847),
                    _ => None,
                };
                if let Some(id) = id {
                    self.game.intros.easy.call = self.sfx_play(id, 0x7F, 0x40);
                }
            }
            0x64 => {
                let call = self.game.intros.easy.call;
                if call != -1 {
                    self.sfx_stop(call);
                }
                self.preload_character_sfx(u32::from(d.enemy_kinds[0]));
            }
            5 => {
                for i in 0..usize::from(d.allies) {
                    let t = self.game.intros.easy.texts[1 + i].expect("a name's text");
                    let (sx, sy) = (
                        self.easy_row(d.allies, 0x18, i),
                        self.easy_row(d.allies, 0x24, i),
                    );
                    if i != 0 || d.nametag == 0x78 {
                        self.intro_name(t, d.ally_kinds[i], false, sx, sy);
                    } else {
                        let name = self.game.name_text(d.nametag);
                        let name = name.map(|mut n| {
                            n.push(0);
                            n
                        });
                        self.game
                            .sis
                            .replace_string(self.data, t, 0, name.as_deref());
                        self.game.sis.set_scale(self.data, t, 0, 1.0, 1.0);
                    }
                }
            }
            6 => {
                if d.kind == 4 {
                    super::super::code::unported_call("Classic's horde stage's name (its count)");
                }
                for i in 0..usize::from(d.enemies) {
                    let t = self.game.intros.easy.texts[4 + i].expect("a name's text");
                    let (sx, sy) = (
                        self.easy_row(d.enemies, 0x18, i),
                        self.easy_row(d.enemies, 0x24, i),
                    );
                    self.intro_name(t, d.enemy_kinds[i], true, sx, sy);
                }
            }
            _ => {}
        }
    }

    /// `fn_8018504C`, the map's process: its two frames counted (the first looping at 50,
    /// the second held at 49), its joints 1 and 0xE animated at them past the stage's 50
    /// frames; at tick 45 on a bonus stage, its game type's sound.
    #[doc(alias = "fn_8018504C")]
    pub fn intro_easy_map_proc(&mut self) {
        let s = &mut self.game.intros.easy;
        if s.map_frame < 0x31 {
            s.map_frame += 1;
        } else {
            s.map_frame = 0;
        }
        if s.map_frame2 < 0x31 {
            s.map_frame2 += 1;
        }
        let base = i32::from(s.data.stage) * 0x32 - 0x32;
        let f5 = mem(fp::frsp(f64::from(i32::from(s.map_frame2) + base)));
        let f4 = mem(fp::frsp(f64::from(i32::from(s.map_frame) + base)));
        let (j5, j4) = (s.joints[5], s.joints[4]);
        let (tick, kind, game_type) = (s.tick, s.data.kind, s.data.game_type);
        let objs = &mut self.game.objs;
        jobj::req_anim_all(objs, j5, f5);
        jobj::anim_all(objs, self.data, &mut ParticleHooks, j5);
        jobj::req_anim_all(objs, j4, f4);
        jobj::anim_all(objs, self.data, &mut ParticleHooks, j4);
        if tick == 0x2D && kind == 3 {
            let id = match game_type {
                1 => Some(0x7C864),
                2 => Some(0x9C49),
                3 => Some(0x9C47),
                _ => None,
            };
            if let Some(id) = id {
                self.sfx_play(id, 0x7F, 0x40);
            }
        }
    }

    /// `fn_80160DE8`: a character's name into text `t`'s first string (its other name with
    /// `alt`, where it has one), kerned in English, at its name's width scale times `sx`
    /// and `sy`.
    #[doc(alias = "fn_80160DE8")]
    #[doc(alias = "fn_801609E0")]
    fn intro_name(&mut self, t: TextId, ckind: u8, alt: bool, sx: f32, sy: f32) {
        let us = self.game.game_data.saved_language() == super::main_lib::LANG_US;
        if us {
            self.game.sis.text_mut(t).default_kerning = 1;
        }
        let other = if alt {
            super::names::other_name(us, ckind)
        } else {
            None
        };
        let name = other.or_else(|| self.game.character_name(ckind));
        let size = super::names::name_width_scale(us, ckind, other.is_some());
        let mut s = name.map(|n| n.to_vec()).unwrap_or_default();
        s.push(0);
        self.game.sis.replace_string(self.data, t, 0, Some(&s));
        let x = fp::fmuls(reg(size), reg(sx));
        self.game.sis.set_scale(self.data, t, 0, x, f64::from(sy));
    }
}

impl Running<'_> {
    /// `gm_Scene_IntroNormal_OnEnter`, Adventure's intro: the player's port, the dynamics',
    /// collisions' and stage's setups with the stage the enter data names (no players), the
    /// items' and effects', the background flash, the state reset, `IrNml`'s scene and the
    /// stage's camera, the stage's song's banks, the stage's camera flying (`fn_80187910`),
    /// the scene's camera, its light, the map (`fn_80187AB4`) and the title (`fn_80187CF4`)
    /// drawn over everything, and the intro's song.
    #[doc(alias = "gm_Scene_IntroNormal_OnEnter")]
    pub fn intro_normal_on_enter(&mut self) {
        // The scene takes the files the preload cache holds (`lbArchive_80017040`).
        self.game.ft.data.files.from_cache = true;
        let e = self.game.one_player.adventure_intro;
        self.game.intros.normal.slot = e.slot;
        self.game.alloc_dynamics(self.data);
        self.game.coll.reset();
        super::super::gr::setup::prepare(self.game);
        let stkind = i32::from(e.stkind);
        let grkind = crate::lb::audio::ground_kind(stkind) as i32;
        let us = self.files_us();
        let archive = super::super::gr::data::stage_file(grkind, us).map(|name| {
            self.game
                .ft
                .data
                .files
                .archive(name.trim_start_matches('/'))
        });
        super::super::gr::setup::select(self, stkind, archive);
        let kept = self.take_cached_named("ItCo.usd");
        let it_co = self.game.ft.data.files.archive("ItCo.usd");
        super::super::it::item::load_common_data(self.game, self.data, it_co, kept);
        super::super::it::item::init_globals(self.game, self.data);
        super::super::ef::eflib::init(self);
        super::super::ef::load_sync(self, 0);
        super::super::ef::load_sync(self, 0x1F);
        let grkind = self.game.stage_info.grkind;
        super::super::gr::setup::setup_at(self, grkind, 0x10);
        self.game.init_all_players();
        self.bg_flash_create(10);
        let s = &mut self.game.intros.normal;
        s.map_anim = 0;
        s.stretch = 0;
        s.x34 = 0;
        s.done = false;
        s.flash = false;
        s.stage = e.stage & 0x1F;
        s.title_anim = 0;
        s.active = false;
        let si = usize::from(e.stage);
        let name = self.intro_file_name(NORMAL_ARCHIVE);
        let (a, file, block) = self.load_archive_blocks(&name);
        let sym = |r: &Self, s: &str| {
            let offset = r
                .data
                .get(a)
                .public(s)
                .unwrap_or_else(|| panic!("{s} isn't in {name}"));
            DataRef { archive: a, offset }
        };
        let scene = sym(self, NORMAL_SCENE_SYMBOL);
        let camera = sym(self, NORMAL_CAMERAS[si]);
        let s = &mut self.game.intros.normal;
        s.scene = Some(scene);
        s.camera = Some(camera);
        s.archive = Loaded {
            archive: Some(a),
            file,
            block,
        };
        let song = NORMAL_SONGS[si];
        let ckind = self.game.player(0).ckind;
        let mask = crate::lb::audio::character_banks(ckind) | NORMAL_BANKS[si];
        self.game.audio.lb.unwant_banks(song as u32);
        self.game.audio.lb.want_banks(song as u32, mask);
        self.game.audio.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
        self.intro_normal_cameras(scene, camera);
        self.intro_normal_light(scene);
        self.intro_normal_models(scene);
        self.lb_play_music(0x2E);
    }

    /// The stage's camera (its process `fn_80187910`, drawn by `Camera_800304E0`, its
    /// animation from frame 0), then the scene's first camera drawing render links 10 and
    /// 11.
    fn intro_normal_cameras(&mut self, scene: DataRef, camera: DataRef) {
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_CAMERA, 0x14, 0);
        g.intros.normal.gobj = Some(gobj);
        let c = crate::hsd::cobj::load_desc(&mut g.objs, data, data.ptr(camera))
            .expect("the stage's camera");
        g.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(gobj, CodeRef(NORMAL_STAGE_CAMERA_RENDER), 8, false);
        g.world.setup_proc(gobj, CodeRef(NORMAL_CAMERA_PROC), 0);
        let anim = data.ptr(camera.at(4)).and_then(|a| data.ptr(a));
        crate::hsd::cobj::add_anim(&mut g.objs, data, Some(c), anim);
        crate::hsd::cobj::req_anim(&mut g.objs, Some(c), 0.0);
        let gobj = g.world.create(CLASS_CAMERA, 0x14, 0);
        let cams = data.ptr(scene.at(4)).expect("the scene's cameras");
        let c = crate::hsd::cobj::load_desc(&mut g.objs, data, data.ptr(cams))
            .expect("the scene's camera");
        g.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(gobj, CodeRef(CAMERA_CALLBACK), 8, false);
        g.world.gobj_mut(gobj).gxlink_prios = 0xC00;
    }

    /// The scene's lights (`lb_80011AC4`) on a light GObj drawn in render link 10.
    fn intro_normal_light(&mut self, scene: DataRef) {
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_LIGHT, 3, 0);
        let lights = data.ptr(scene.at(8)).expect("the scene's lights");
        let l = super::super::gr::setup::load_light_list(g, data, lights);
        g.world
            .set_obj(gobj, OBJ_LIGHT, Obj::LObj(l.expect("a light")));
        g.world.setup_gx_link(gobj, CodeRef(LOBJ_CALLBACK), 0xA, 0);
    }

    /// The scene's model `index` on a GObj, its materials' render mode with 0x0800_0000
    /// (`lb_80011C18`), drawn in render link 11 without the depth test (`fn_80187C9C`).
    fn intro_normal_model(&mut self, scene: DataRef, index: u32) -> (GObjId, JObjId, DataRef) {
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_UI, 0xF, 0);
        let models = data.ptr(scene).expect("the scene's models");
        let m = data.ptr(models.at(4 * index)).expect("a model");
        let j = jobj::load_joint(&mut g.objs, data, data.ptr(m)).expect("a model's joint");
        crate::lb::joint::set_mobj_flags(&mut g.objs, j, 0x0800_0000);
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world
            .setup_gx_link(gobj, CodeRef(NORMAL_MODEL_RENDER), 0xB, 0xB);
        (gobj, j, m)
    }

    /// The map (the scene's model 11 less the stage's number) at its first animation, and the
    /// title (model 12) at its, each with its process.
    fn intro_normal_models(&mut self, scene: DataRef) {
        let stage = u32::from(self.game.intros.normal.stage);
        let (gobj, j, m) = self.intro_normal_model(scene, 11 - stage);
        let k = self.game.intros.normal.map_anim;
        self.intro_normal_anim(j, m, k);
        self.game
            .world
            .setup_proc(gobj, CodeRef(NORMAL_MAP_PROC), 0);
        let (gobj, j, m) = self.intro_normal_model(scene, 12);
        let k = self.game.intros.normal.title_anim;
        self.intro_normal_title_anim(j, m, k);
        jobj::req_anim_all(&mut self.game.objs, Some(j), 0.0);
        self.anim_all(j);
        self.game
            .world
            .setup_proc(gobj, CodeRef(NORMAL_TITLE_PROC), 0);
    }

    /// The map's animation `k` started when it has animations and that one
    /// (`fn_80187AB4_LoadAnim`).
    fn intro_normal_anim(&mut self, j: JObjId, m: DataRef, k: u8) {
        if self.data.ptr(m.at(4)).is_some() {
            self.intro_normal_title_anim(j, m, k);
        }
    }

    /// The title's animation `k` started when it has that one (its list is read without a
    /// check).
    fn intro_normal_title_anim(&mut self, j: JObjId, m: DataRef, k: u8) {
        let anims = self.data.ptr(m.at(4)).expect("the model's animations");
        if self.data.ptr(anims.at(4 * u32::from(k))).is_some() {
            crate::lb::anim::add_model_anim(
                &mut self.game.objs,
                self.data,
                Some(j),
                u32::from(k),
                m,
            );
            jobj::req_anim_all(&mut self.game.objs, Some(j), 0.0);
            self.anim_all(j);
        }
    }

    /// `gm_Scene_IntroNormal_OnFrame`: the scene's end once the map asked for it.
    #[doc(alias = "gm_Scene_IntroNormal_OnFrame")]
    pub fn intro_normal_on_frame(&mut self) {
        if self.game.intros.normal.done {
            self.game.end_scene();
        }
    }

    /// `gm_Scene_IntroNormal_OnLeave`: `IrNml`'s blocks back to the scene's heap.
    #[doc(alias = "gm_Scene_IntroNormal_OnLeave")]
    pub fn intro_normal_on_leave(&mut self) {
        let a = self.game.intros.normal.archive;
        self.free_archive_blocks(a.file, a.block);
    }

    /// `fn_80187910`, the stage's camera's process: A moves it on to the start of its next
    /// stretch of 300 frames (of eight); it animates, then its interest and eye scale by the
    /// stage's scale (`Ground_801C0498`).
    #[doc(alias = "fn_80187910")]
    pub fn intro_normal_camera_proc(&mut self, gobj: GObjId) {
        use crate::hsd::cobj;
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("the intro's camera holds no CObj");
        };
        let slot = self.game.intros.normal.slot;
        if self.game.buttons_triggered(slot) & u64::from(crate::input::pad::BUTTON_A) != 0 {
            let objs = &self.game.objs;
            let eye = objs.cobjs[c].eyepos.expect("a camera without an eye");
            let a = objs.wobjs[eye.0].aobj.expect("an eye without an animation");
            let frame = objs.aobjs[a].curr_frame;
            let s = &mut self.game.intros.normal;
            s.stretch = ((fp::fctiwz(reg(frame)) / 300) as u8) & 0xF;
            s.stretch = (s.stretch + 1) & 0xF;
            if s.stretch >= 8 {
                s.stretch = 0;
            }
            let frame = i32::from(s.stretch) * 300;
            let camera = s.camera.expect("the stage's camera");
            let anim = self.data.ptr(camera.at(4)).and_then(|a| self.data.ptr(a));
            cobj::remove_anim(&mut self.game.objs, Some(c));
            cobj::add_anim(&mut self.game.objs, self.data, Some(c), anim);
            cobj::req_anim(
                &mut self.game.objs,
                Some(c),
                mem(fp::frsp(f64::from(frame))),
            );
        }
        cobj::anim(&mut self.game.objs, self.data, Some(c));
        let scaled = |v: crate::vec::Vec3, k: f32| {
            let k = reg(k);
            crate::vec::Vec3::new(
                mem(fp::fmuls(reg(v.x), k)),
                mem(fp::fmuls(reg(v.y), k)),
                mem(fp::fmuls(reg(v.z), k)),
            )
        };
        let v = cobj::interest(&mut self.game.objs, self.data, c).expect("an interest");
        let k = super::super::gr::ground::scale(self.game);
        cobj::set_interest(&mut self.game.objs, c, scaled(v, k));
        let v = cobj::eye_position(&mut self.game.objs, self.data, c).expect("an eye");
        let k = super::super::gr::ground::scale(self.game);
        cobj::set_eye_position(&mut self.game.objs, c, scaled(v, k));
    }

    /// `fn_80187AB4`, the map's process: its first animation played out, the second
    /// (repeating) until Start, which fades the screen to black and asks for the scene's
    /// end; then the third played out, the end asked for again and the map freed.
    #[doc(alias = "fn_80187AB4")]
    pub fn intro_normal_map_proc(&mut self, gobj: GObjId) {
        let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj else {
            panic!("the intro's map holds no JObj");
        };
        let scene = self.game.intros.normal.scene.expect("the intro's scene");
        let stage = u32::from(self.game.intros.normal.stage);
        let models = self.data.ptr(scene).expect("the scene's models");
        let m = self.data.ptr(models.at(4 * (11 - stage))).expect("the map");
        let animated = crate::lb::anim::any_animated(&self.game.objs, Some(j));
        match self.game.intros.normal.map_anim {
            0 => {
                if !animated {
                    self.game.intros.normal.map_anim = 1;
                    self.intro_normal_anim(j, m, 1);
                    self.game.intros.normal.x34 = 0x960;
                }
            }
            1 => {
                let s = &mut self.game.intros.normal;
                if s.active {
                    s.flash = true;
                }
                if s.flash {
                    s.map_anim = 2;
                    s.done = true;
                    self.game.bgflash.fade_to_black(1);
                }
                if !animated {
                    let k = self.game.intros.normal.map_anim;
                    self.intro_normal_anim(j, m, k);
                }
            }
            2 if !animated => {
                self.game.intros.normal.done = true;
                crate::hsd::gobj::free(self, gobj);
            }
            _ => {}
        }
        self.anim_all(j);
    }

    /// `fn_80187CF4`, the title's process: its first two animations played out, the third
    /// repeating until Start (the forward sound, the map's flash asked for), then the fourth
    /// played out and the title freed.
    #[doc(alias = "fn_80187CF4")]
    pub fn intro_normal_title_proc(&mut self, gobj: GObjId) {
        let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj else {
            panic!("the intro's title holds no JObj");
        };
        let scene = self.game.intros.normal.scene.expect("the intro's scene");
        let models = self.data.ptr(scene).expect("the scene's models");
        let m = self.data.ptr(models.at(4 * 12)).expect("the title");
        let animated = crate::lb::anim::any_animated(&self.game.objs, Some(j));
        match self.game.intros.normal.title_anim {
            k @ (0 | 1) => {
                if !animated {
                    self.game.intros.normal.title_anim = k + 1;
                    self.intro_normal_title_anim(j, m, k + 1);
                }
            }
            2 => {
                let slot = self.game.intros.normal.slot;
                let start = u64::from(crate::input::pad::BUTTON_START);
                if self.game.buttons_triggered(slot) & start != 0 {
                    self.lb_audio_call(1);
                    self.game.intros.normal.active = true;
                    self.game.intros.normal.title_anim = 3;
                    self.intro_normal_title_anim(j, m, 3);
                } else if !animated {
                    self.intro_normal_title_anim(j, m, 2);
                }
            }
            _ => {
                if !animated {
                    crate::hsd::gobj::free(self, gobj);
                }
            }
        }
        self.anim_all(j);
    }

    /// `fn_80187C9C`, the map's and the title's render callback: the depth test and its
    /// writes off at the hardware (`GXSetZMode`, past HAL's state cache), the joint tree,
    /// then the cache's depth mode out of date (`HSD_StateInvalidate(0x40)`).
    #[doc(alias = "fn_80187C9C")]
    pub fn intro_normal_model_render(&mut self, gobj: GObjId, pass: i32) {
        use crate::hsd::display::Draw;
        self.game.objs.display.draws.push(Draw::ZModeDirect {
            compare: false,
            // `GX_NEVER`.
            func: 0,
            update: false,
        });
        self.render_jobj(gobj, pass);
        self.game.objs.display.draws.push(Draw::Invalidate(0x40));
    }

    /// `Camera_800304E0`, the stage's camera's render callback: through the camera, the
    /// screen erased to the game camera's background color, the stage's fog fitted, the
    /// current lights dropped, then the game camera's layers 2, 1 and 0 over render links 0
    /// and 3, links 4 to 6, fog off, and link 7.
    #[doc(alias = "Camera_800304E0")]
    pub fn intro_normal_stage_render(&mut self, gobj: GObjId) {
        use super::super::camera;
        use crate::hsd::display::{self, Draw};
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("the intro's camera holds no CObj");
        };
        if !display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
            return;
        }
        // `HSD_SetEraseColor`: the game camera's background color, its alpha 1.
        let bg = &self.game.camera.cam.background_color;
        let color = [bg.r, bg.g, bg.b, 1];
        let draws = &mut self.game.objs.display.draws;
        draws.push(Draw::EraseColor(color));
        draws.push(Draw::Erase);
        self.stage_fog_update(c);
        crate::hsd::lobj::delete_current_all(&mut self.game.objs, None);
        let prio8 = |r: &Self| if r.game.camera.cam.x398_b2 { 0 } else { 8 };
        let prio1 = if self.game.camera.cam.x398_b4 { 0 } else { 1 };
        camera::set_layer(&mut self.game.camera.cam, 2);
        self.game.world.gobj_mut(gobj).gxlink_prios = prio1 | prio8(self);
        self.render_links(gobj, 7);
        for layer in [1, 0] {
            camera::set_layer(&mut self.game.camera.cam, layer);
            self.game.world.gobj_mut(gobj).gxlink_prios = prio8(self);
            self.render_links(gobj, 7);
        }
        self.game.world.gobj_mut(gobj).gxlink_prios = 0x70;
        self.render_links(gobj, 7);
        display::fog_set(&mut self.game.objs, None);
        self.game.world.gobj_mut(gobj).gxlink_prios = 0x80;
        self.render_links(gobj, 7);
        display::camera_end(&mut self.game.objs);
    }
}

impl Running<'_> {
    /// The files the intro about to enter reads, for the frontend: its archives, its demo
    /// fighters (their characters and costumes as a match's start data names them) and their
    /// intro animations (pose 5's and 6's, the demo animations 10 and 11), and its song.
    pub fn intro_scene_files(&self) -> Option<super::super::vi::SceneFiles> {
        let scene = self.entering_scene()?;
        if !is_intro(scene) {
            return None;
        }
        let mut f = super::super::vi::SceneFiles::default();
        if scene == GS_INTRO_NORMAL {
            // Adventure's: `IrNml`, the stage's file and the intro's song.
            f.names.push(self.intro_file_name(NORMAL_ARCHIVE));
            let stkind = i32::from(self.game.one_player.adventure_intro.stkind);
            let grkind = crate::lb::audio::ground_kind(stkind) as i32;
            if let Some(name) = super::super::gr::data::stage_file(grkind, self.files_us()) {
                f.names.push(name.trim_start_matches('/').to_owned());
            }
            f.songs.push(0x2E);
            return Some(f);
        }
        let e = self.intro_easy_enter();
        let mut d = EasyData::default();
        d.copy_enter(&e);
        if d.enemy_kinds[0] == CKIND_MASTERH || d.enemy_kinds[1] == CKIND_MASTERH {
            return Some(f);
        }
        let archive = self.intro_file_name(GAME_TYPE_ARCHIVES[d.game_type as usize]);
        f.names = vec![
            TABLE_ARCHIVE.to_owned(),
            archive.clone(),
            self.intro_file_name(MAP_ARCHIVE),
            "SdIntro.dat".to_owned(),
        ];
        f.songs.push(0x2D);
        if !(0..=2).contains(&d.kind) {
            return Some(f);
        }
        let mut slot = 0;
        let mut add = |f: &mut super::super::vi::SceneFiles, ckind: u8, costume: u8| {
            let p = &mut f.players.players[slot];
            p.ckind = ckind as i8;
            p.color = costume;
            p.slot_type = super::super::pl::player::PKIND_DEMO as u8;
            slot += 1;
            let m = super::super::pl::player::mapping(i32::from(ckind));
            for kind in [m.internal_id, m.extra_internal_id] {
                if kind < 0 || (kind == m.extra_internal_id && m.has_transformation != 0) {
                    continue;
                }
                f.anims.push((archive.clone(), i32::from(kind), 1, 10, 11));
            }
        };
        for i in 0..usize::from(d.allies) {
            if d.ally_kinds[i] != NONE {
                add(&mut f, d.ally_kinds[i], d.ally_colors[i]);
            }
        }
        for i in 0..usize::from(d.enemies) {
            let c = if d.enemy_kinds[i] == CKIND_GKOOPS {
                CKIND_KOOPA
            } else {
                d.enemy_kinds[i]
            };
            if d.ally_kinds[i] != NONE && c != NONE {
                add(&mut f, c, d.enemy_colors[i]);
            }
        }
        Some(f)
    }
}

/// `HSD_JObjAddTranslationZ`.
fn add_z(jobjs: &mut crate::hsd::jobj::JObjs, j: JObjId, z: f32) {
    let tz = jobjs[j].translate.z;
    jobj::set_srt(jobjs, j, |o| {
        o.translate.z = mem(fp::fadds(reg(tz), reg(z)))
    });
}

/// A joint's first display object's material's first texture (`jobj->u.dobj->mobj->tobj`).
fn digit_tobj(objs: &crate::hsd::Objects, j: JObjId) -> Option<crate::hsd::tobj::TObjId> {
    let d = objs.jobjs[j].dobj().expect("jobj->u.dobj");
    let m = objs.dobjs[d].mobj.expect("dobj->mobj");
    objs.mobjs[m].tobj
}

/// Runs an intro's process, if `f` is one.
pub fn run_proc(r: &mut Running, gobj: GObjId, f: u32) -> bool {
    match f {
        EASY_MODEL_PROC => r.intro_easy_model_proc(gobj),
        EASY_MAP_PROC => r.intro_easy_map_proc(),
        NORMAL_CAMERA_PROC => r.intro_normal_camera_proc(gobj),
        NORMAL_MAP_PROC => r.intro_normal_map_proc(gobj),
        NORMAL_TITLE_PROC => r.intro_normal_title_proc(gobj),
        f if EASY_FIGHTER_PROCS.contains(&f) => {
            let i = EASY_FIGHTER_PROCS.iter().position(|&p| p == f).unwrap();
            if i < 3 {
                r.intro_easy_ally_proc(gobj, i);
            } else {
                r.intro_easy_enemy_proc(gobj, i - 3);
            }
        }
        _ => return false,
    }
    true
}

/// Runs an intro's frame function, if `f` is one.
pub fn run_frame(r: &mut Running, f: u32) -> bool {
    match f {
        f if f == EASY_ON_FRAME.0 => r.intro_easy_on_frame(),
        f if f == NORMAL_ON_FRAME.0 => r.intro_normal_on_frame(),
        _ => return false,
    }
    true
}

/// Runs an intro's render callback, if `cb` is one.
pub fn run_render(r: &mut Running, gobj: GObjId, cb: u32, pass: i32) -> bool {
    match cb {
        EASY_CAMERA_RENDER => r.intro_easy_camera_render(gobj),
        NORMAL_MODEL_RENDER => r.intro_normal_model_render(gobj, pass),
        NORMAL_STAGE_CAMERA_RENDER => r.intro_normal_stage_render(gobj),
        _ => return false,
    }
    true
}

impl Running<'_> {
    /// `fn_8018575C`, the map's camera's render callback: through the camera, the fighters'
    /// passes (`Camera_800313E0`), then render links 0 and 12.
    #[doc(alias = "fn_8018575C")]
    pub fn intro_easy_camera_render(&mut self, gobj: GObjId) {
        let c = match self.game.world.gobj(gobj).obj {
            Some(Obj::CObj(c)) => Some(c),
            other => panic!("the intro's camera holds {other:?}"),
        };
        if crate::hsd::display::camera_begin(&mut self.game.objs, self.data, c) {
            self.camera_fighters(gobj, 1);
            self.game.world.gobj_mut(gobj).gxlink_prios = 0x1001;
            self.render_links(gobj, 7);
            crate::hsd::display::camera_end(&mut self.game.objs);
        }
    }
}
