//! The title screen (`melee/gm/gmtitle.c`, `gmtitlemode.c`): the title's archive
//! (`GmTtAll`), its camera, light, fog and erase, the logo (`TtlMoji`, with the emblem only once
//! Marth or Roy is unlocked) and the animated background (`TtlBg`). The opening movie shows the
//! same logo over its last frames, then the title mode shows it with the background until
//! Start or a timeout.
//!
//! Making the logo draws the random seed once for each second of the real-time clock's
//! current minute, then picks the attract demo's match ([`Running::setup_title_demo`]): both
//! reach gameplay, so the clock is an input of the session's.

use melee_data::archive::DataRef;

use super::super::handles::CodeRef;
use super::super::render::{FOG_CALLBACK, JOBJ_CALLBACK, LOBJ_CALLBACK};
use super::super::state::{OBJ_CAMERA, OBJ_FOG, OBJ_JOBJ, OBJ_LIGHT, Obj, Running};
use super::machine::{GM_OPENING_MV, GM_TITLE};
use crate::hsd::fog;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

/// `HSD_GOBJ_CLASS_UI`, `HSD_GOBJ_CLASS_CAMERA`, `HSD_GOBJ_CLASS_LIGHT`, `HSD_GOBJ_CLASS_FOG`.
pub const CLASS_UI: u16 = 14;
pub const CLASS_CAMERA: u16 = 19;
const CLASS_LIGHT: u16 = 11;
const CLASS_FOG: u16 = 10;

/// The title's processes and render callbacks.
pub const BG_PROC: u32 = 0x801A_146C;
pub const LOGO_PROC: u32 = 0x801A_1498;
pub const LOGO_STILL_PROC: u32 = 0x801A_1630;
pub const CAMERA_RENDER: u32 = 0x801A_1814;
pub const ERASE_RENDER: u32 = 0x801A_18D4;
pub const FOG_PROC: u32 = 0x801A_1A18;
/// `gm_Scene_Title_OnEnter` and `gm_Scene_Title_OnFrame`.
pub const ON_ENTER: CodeRef = CodeRef(0x801A_1E20);
pub const ON_FRAME: CodeRef = CodeRef(0x801A_1C18);

/// `AnimLoopSettings`: an animation's start, end and loop frames (no loop at -0.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimLoop {
    pub start: f32,
    pub end: f32,
    pub loop_frame: f32,
}

/// `loop_settings_0` (the logo's) and `loop_settings_1` (the background's).
const LOGO_LOOP: AnimLoop = AnimLoop {
    start: 0.0,
    end: 1600.0,
    loop_frame: 400.0,
};
const BG_LOOP: AnimLoop = AnimLoop {
    start: 0.0,
    end: 1330.0,
    loop_frame: 130.0,
};

/// A model's joint and animations (`StaticModelDesc`), from the title's archive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct ModelDesc {
    pub joint: Option<DataRef>,
    pub anim: Option<DataRef>,
    pub matanim: Option<DataRef>,
    pub shapeanim: Option<DataRef>,
}

/// `gmtitle.c`'s and `gmtitlemode.c`'s statics: the archive's symbols, the scene's timers, and
/// the title mode's exit data (the buttons that ended it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Title {
    pub logo: ModelDesc,
    pub bg: ModelDesc,
    pub cobj_desc: Option<DataRef>,
    pub lights: Option<DataRef>,
    pub fog_desc: Option<DataRef>,
    /// `gm_804D67F0`, `TitleMark_sobjdesc` (gmopening.c's, set with the others).
    pub mark: Option<DataRef>,
    /// `countdown_timer`, `frame_count`, `bg_initialized`.
    pub countdown: i32,
    pub frames: u32,
    pub bg_made: bool,
    /// The title mode's `exit_data.buttons`.
    pub exit_buttons: i32,
}

/// `mn_8022F298`: the frame of the first animation found in the joint tree (its joint's, its
/// first material's or that material's first texture's), -1 for none.
#[doc(alias = "mn_8022F298")]
pub fn anim_frame(r: &Running, j: JObjId) -> f32 {
    let o = &r.game.objs;
    let jo = &o.jobjs[j];
    if let Some(a) = jo.aobj {
        return o.aobjs[a].curr_frame;
    }
    if let Some(d) = jo.dobj()
        && let Some(m) = o.dobjs[d].mobj
    {
        if let Some(a) = o.mobjs[m].aobj {
            return o.aobjs[a].curr_frame;
        }
        if let Some(t) = o.mobjs[m].tobj
            && let Some(a) = o.tobjs[t].aobj
        {
            return o.aobjs[a].curr_frame;
        }
    }
    if jo.flags & jobj::INSTANCE == 0 {
        let mut c = jo.child;
        while let Some(n) = c {
            let f = anim_frame(r, n);
            if f != -1.0 {
                return f;
            }
            c = o.jobjs[n].next;
        }
    }
    -1.0
}

impl Running<'_> {
    fn anim_all(&mut self, j: JObjId) {
        jobj::anim_all(
            &mut self.game.objs,
            self.data,
            &mut super::super::code::ParticleHooks,
            Some(j),
        );
    }

    /// `mn_8022ED6C`: a joint tree's animation one frame on, looping from `l.loop_frame` past
    /// `l.end` (or stopping there); the frame it reached.
    #[doc(alias = "mn_8022ED6C")]
    pub fn anim_loop(&mut self, j: JObjId, l: AnimLoop) -> f64 {
        use gekko_fp as fp;
        let reg = |v: f32| crate::fpu::reg(v);
        let cur = f64::from(anim_frame(self, j));
        if !(reg(l.start) <= cur && cur <= reg(l.end)) {
            jobj::req_anim_all(&mut self.game.objs, Some(j), l.start);
        }
        if reg(l.loop_frame) == fp::fneg(fp::frsp(0.1)) {
            let mut cur = f64::from(anim_frame(self, j));
            if cur < reg(l.end) {
                self.anim_all(j);
                cur = f64::from(anim_frame(self, j));
                if cur > reg(l.end) {
                    cur = reg(l.end);
                    jobj::req_anim_all(&mut self.game.objs, Some(j), cur as f32);
                    self.anim_all(j);
                }
            }
            cur
        } else {
            self.anim_all(j);
            let mut cur = f64::from(anim_frame(self, j));
            let end = reg(l.end);
            if cur >= end {
                cur = fp::fadds(reg(l.loop_frame), fp::fsubs(cur, end));
                jobj::req_anim_all(&mut self.game.objs, Some(j), cur as f32);
                self.anim_all(j);
            }
            cur
        }
    }

    fn title_jobj(&self, g: GObjId) -> JObjId {
        match self.game.world.gobj(g).obj {
            Some(Obj::JObj(j)) => j,
            other => panic!("a title GObj holds {other:?}"),
        }
    }

    /// A model GObj of the UI class on p-link 15: its tree loaded, drawn by
    /// `HSD_GObj_JObjCallback` on render link `link`, its animations added.
    fn title_model(&mut self, m: ModelDesc, link: u8) -> (GObjId, JObjId) {
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_UI, 15, 0);
        let j = jobj::load_joint(&mut g.objs, data, m.joint).expect("a title model");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), link, 0);
        jobj::add_anim_all(&mut g.objs, data, Some(j), m.anim, m.matanim, m.shapeanim);
        (gobj, j)
    }

    /// Hides the subtree at pre-order index `index` (`lb_80011E24`, `HSD_JObjSetFlagsAll`).
    fn hide_node(&mut self, j: JObjId, index: i32) {
        let [n] = super::pause::tree_nodes(&self.game.objs.jobjs, Some(j), &[index]);
        jobj::set_flags_all(&mut self.game.objs.jobjs, n, jobj::HIDDEN);
    }

    /// `gmTitle_801A12C4`: the logo the movie flashes early on, at its loop frame, moved down
    /// and without its subtitle and emblem.
    #[doc(alias = "gmTitle_801A12C4")]
    pub fn title_flash_logo(&mut self) -> GObjId {
        let logo = self.game.title.logo;
        let (gobj, j) = self.title_model(logo, 9);
        jobj::req_anim_all(&mut self.game.objs, Some(j), LOGO_LOOP.loop_frame);
        self.anim_all(j);
        jobj::set_translate(&mut self.game.objs.jobjs, j, Vec3::new(0.0, -3.0, 0.0));
        self.hide_node(j, 3);
        self.hide_node(j, 1);
        gobj
    }

    /// Whether the logo plays from its start (`isActiveTitle`): neither in the title mode nor
    /// in the opening mode's state 2, the title after the attract match
    /// (`gm_GetCurrentSceneIndex`, the mode state's id, against `GS_VS`, also 2).
    #[doc(alias = "gm_GetCurrentSceneIndex")]
    fn title_active(&self) -> bool {
        let r = &self.game.machine.routing;
        !(r.curr_mode == GM_TITLE || (r.curr_mode == GM_OPENING_MV && r.curr_state_id == 2))
    }

    /// `gmTitle_801A146C`, the background's process.
    #[doc(alias = "gmTitle_801A146C")]
    pub fn title_bg_proc(&mut self, gobj: GObjId) {
        let j = self.title_jobj(gobj);
        self.anim_loop(j, BG_LOOP);
    }

    /// The background (`fn_801A1498_inline`): `TtlBg` on render link 3, its process, from its
    /// start or, in the title mode, from frame 130.
    fn title_bg(&mut self) {
        let bg = self.game.title.bg;
        let (gobj, j) = self.title_model(bg, 3);
        self.game.world.setup_proc(gobj, CodeRef(BG_PROC), 0);
        let frame = if self.title_active() {
            BG_LOOP.start
        } else {
            130.0
        };
        jobj::req_anim_all(&mut self.game.objs, Some(j), frame);
        self.anim_all(j);
    }

    /// `fn_801A1498`, the logo's process in the movie: it follows the movie's clock until the
    /// movie's end, then loops; once its frame reaches 270 the background appears and the
    /// movie stops drawing.
    #[doc(alias = "fn_801A1498")]
    pub fn title_logo_proc(&mut self, gobj: GObjId) {
        let j = self.title_jobj(gobj);
        let t = self.game.opening.clock;
        if t > 5400 {
            self.anim_loop(j, LOGO_LOOP);
        } else {
            let f = t.wrapping_sub(5130);
            jobj::req_anim_all(&mut self.game.objs, Some(j), f as f32);
            self.anim_all(j);
        }
        if !self.game.title.bg_made && anim_frame(self, j) >= 270.0 {
            self.title_bg();
            self.game.audio.movie.set_draw(0);
            self.game.title.bg_made = true;
        }
    }

    /// `gmTitle_801A1630`, the logo's process in the title mode.
    #[doc(alias = "gmTitle_801A1630")]
    pub fn title_logo_still_proc(&mut self, gobj: GObjId) {
        let j = self.title_jobj(gobj);
        self.anim_loop(j, LOGO_LOOP);
    }

    /// `gmTitle_801A165C`: the logo, animated from its start in the movie or from frame 400
    /// in the title mode, with the emblem only once Marth or Roy is unlocked; then the
    /// random seed drawn once for each second of the clock's minute, and the attract demo's
    /// match picked.
    #[doc(alias = "gmTitle_801A165C")]
    #[doc(alias = "gm_801692E8")]
    #[doc(alias = "lbTime_8000B028")]
    pub fn title_logo(&mut self) -> GObjId {
        let logo = self.game.title.logo;
        let (gobj, j) = self.title_model(logo, 9);
        if self.title_active() {
            self.game.title.bg_made = false;
            jobj::req_anim_all(&mut self.game.objs, Some(j), LOGO_LOOP.start);
            self.game.world.setup_proc(gobj, CodeRef(LOGO_PROC), 0);
        } else {
            jobj::req_anim_all(&mut self.game.objs, Some(j), 400.0);
            self.game
                .world
                .setup_proc(gobj, CodeRef(LOGO_STILL_PROC), 0);
        }
        self.anim_all(j);
        // `isEmblemUnlocked`: Marth's (`CKind_Mars`) or Roy's (`CKind_Emblem`).
        let u = &self.game.gm.unlocks;
        if !(u.ckind_unlocked(0x09) || u.ckind_unlocked(0x17)) {
            self.hide_node(j, 7);
        }
        // `gm_801692E8(lbTime_GetTimeInSeconds())`'s second: the clock's seconds past the
        // minute.
        let tb = self.game.audio.tb_read();
        let secs = (tb / crate::audio::clock::TB_HZ).min(u64::from(u32::MAX));
        for _ in 0..secs % 60 {
            self.game.objs.rng.draw();
        }
        self.setup_title_demo();
        gobj
    }

    /// `gmTitle_801A1814`: the title's camera draws its links' three passes.
    #[doc(alias = "gmTitle_801A1814")]
    pub fn title_camera_render(&mut self, gobj: GObjId) {
        self.render_camera(gobj);
    }

    /// `gmTitle_801A185C`: the title's camera (`ScTitle_cam_int1_camera`), drawing render
    /// links 0, 3 and 9.
    #[doc(alias = "gmTitle_801A185C")]
    pub fn title_camera(&mut self) {
        let data = self.data;
        let desc = self.game.title.cobj_desc.expect("the title's camera");
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_CAMERA, 20, 0);
        let c =
            crate::lb::cobj::load_cobj(&mut g.objs, data, crate::lb::cobj::CObjDesc::Data(desc));
        g.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(gobj, CodeRef(CAMERA_RENDER), 0xC, false);
        g.world.gobj_mut(gobj).gxlink_prios = 0x209;
    }

    /// `gmTitle_801A1944`: a camera that erases the screen to the fog's color first.
    #[doc(alias = "gmTitle_801A1944")]
    pub fn title_erase_camera(&mut self) {
        let data = self.data;
        let desc = self.game.title.cobj_desc.expect("the title's camera");
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_CAMERA, 20, 0);
        let c =
            crate::lb::cobj::load_cobj(&mut g.objs, data, crate::lb::cobj::CObjDesc::Data(desc));
        g.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(gobj, CodeRef(ERASE_RENDER), 0, false);
    }

    /// `gmTitle_801A18D4`: the erase camera clears the screen, color and depth, to the fog's
    /// color.
    #[doc(alias = "gmTitle_801A18D4")]
    pub fn title_erase_render(&mut self, gobj: GObjId) {
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("the erase camera holds no CObj");
        };
        let desc = self.game.title.fog_desc.expect("the title's fog");
        // `HSD_FogDesc`'s color, after its type and range adjustment and distances.
        let rgba = self.data.u32(desc.at(0x10)).to_be_bytes();
        let d = crate::hsd::display::camera_begin(&mut self.game.objs, self.data, Some(c));
        if d {
            let draws = &mut self.game.objs.display.draws;
            draws.push(crate::hsd::display::Draw::EraseColor(rgba));
            draws.push(crate::hsd::display::Draw::EraseWithDepth);
            crate::hsd::display::camera_end(&mut self.game.objs);
        }
    }

    /// `gmTitle_801A19AC`: the title's lights.
    #[doc(alias = "gmTitle_801A19AC")]
    pub fn title_lights(&mut self) {
        let data = self.data;
        let list = self.game.title.lights.expect("the title's lights");
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_LIGHT, 3, 128);
        let l = crate::game::gr::setup::load_light_list(g, data, list);
        g.world
            .set_obj(gobj, OBJ_LIGHT, Obj::LObj(l.expect("a light")));
        g.world.setup_gx_link(gobj, CodeRef(LOBJ_CALLBACK), 0, 0);
    }

    /// `gmTitle_801A1A18`, the fog's process.
    #[doc(alias = "gmTitle_801A1A18")]
    pub fn title_fog_proc(&mut self, gobj: GObjId) {
        let Some(Obj::Fog(f)) = self.game.world.gobj(gobj).obj else {
            panic!("the fog GObj holds no fog");
        };
        fog::interpret_anim(&mut self.game.objs, self.data, Some(f));
    }

    /// `gmTitle_801A1A3C`: the title's fog and its process.
    #[doc(alias = "gmTitle_801A1A3C")]
    pub fn title_fog(&mut self) -> GObjId {
        let data = self.data;
        let desc = self.game.title.fog_desc.expect("the title's fog");
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_FOG, 3, 0);
        let f = fog::load_desc(&mut g.objs.fogs, data, desc);
        g.world.set_obj(gobj, OBJ_FOG, Obj::Fog(f));
        g.world.setup_gx_link(gobj, CodeRef(FOG_CALLBACK), 0, 0);
        g.world.setup_proc(gobj, CodeRef(FOG_PROC), 0);
        gobj
    }

    /// `gmTitle_801A1AC0`: the title's archive (`GmTtAll.usd` in English) and its symbols.
    #[doc(alias = "gmTitle_801A1AC0")]
    #[doc(alias = "lbArchive_LoadSymbols")]
    pub fn title_load(&mut self) {
        let name = if super::super::gm::language_setting_is_us() {
            "GmTtAll.usd"
        } else {
            "GmTtAll.dat"
        };
        let a = self.load_archive(name);
        let ar = self.data.get(a);
        let sym = |s: &str| {
            let at = ar
                .public(s)
                .unwrap_or_else(|| panic!("the original's assertion failed: {s} isn't in {name}"));
            DataRef {
                archive: a,
                offset: at,
            }
        };
        let model = |p: &str| ModelDesc {
            joint: Some(sym(&format!("{p}_joint"))),
            anim: Some(sym(&format!("{p}_animjoint"))),
            matanim: Some(sym(&format!("{p}_matanim_joint"))),
            shapeanim: Some(sym(&format!("{p}_shapeanim_joint"))),
        };
        let t = Title {
            logo: model("TtlMoji_Top"),
            bg: model("TtlBg_Top"),
            cobj_desc: Some(sym("ScTitle_cam_int1_camera")),
            lights: Some(sym("ScTitle_scene_lights")),
            fog_desc: Some(sym("ScTitle_fog")),
            mark: Some(sym("TitleMark_sobjdesc")),
            ..self.game.title
        };
        self.game.title = t;
    }

    /// `gm_Scene_Title_OnEnter`: the music stopped, the scene's banks, its fog, lights,
    /// cameras and logo, the attract demo's files preloaded, and the background. (A debug
    /// level shows the build's date: retail's doesn't.)
    #[doc(alias = "gm_Scene_Title_OnEnter")]
    pub fn title_on_enter(&mut self) {
        self.lb_audio_music_stop();
        let t = &mut self.game.title;
        t.countdown = 20;
        t.frames = 0;
        self.title_load();
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x12);
        a.lb.want_banks(2, 4);
        a.lb_start_bank_loads(self.data);
        self.title_fog();
        self.title_lights();
        self.title_erase_camera();
        self.title_camera();
        self.title_logo();
        self.game.audio.lb_wait_for_banks(self.data);
        self.preload_title_demo();
        self.title_bg();
        self.game.assert_retail();
    }

    /// `gm_Scene_Title_OnFrame`: after 20 ticks, Start ends the scene for the main menu (the
    /// menu's banks loading and its song picked), else 600 ticks end it for the attract loop.
    /// A debug level's A, X and Y exits aren't retail's.
    #[doc(alias = "gm_Scene_Title_OnFrame")]
    #[doc(alias = "gmMainLib_8015ECBC")]
    pub fn title_on_frame(&mut self) {
        let input = self.game.buttons_triggered(4) as u32;
        let t = &mut self.game.title;
        if t.countdown != 0 {
            t.countdown -= 1;
            return;
        }
        t.frames += 1;
        if t.frames > 600 {
            t.exit_buttons = 0;
            self.game.end_scene();
        } else if input & u32::from(crate::input::pad::BUTTON_START) != 0 {
            let a = &mut self.game.audio;
            a.lb.unwant_banks(0x1C);
            a.lb.want_banks(0xC, 0);
            a.lb_start_bank_loads(self.data);
            self.game.audio.lb_wait_for_banks(self.data);
            self.lb_audio_call(1);
            self.pick_menu_song();
            self.game.title.exit_buttons = input as i32;
            self.game.end_scene();
        } else {
            self.game.assert_retail();
        }
    }

    /// `gmMainLib_8015ECBC`: the menus' song, the alternate one in one draw of four once
    /// everything is unlocked.
    fn pick_menu_song(&mut self) {
        let all = self.game.gm.unlocks.all_stages() && self.game.gm.unlocks.all_characters();
        let bgm = if all {
            if self.game.objs.rng.below(4) != 0 {
                0x34
            } else {
                0x36
            }
        } else {
            0x34
        };
        self.game.game_data.x1850.bgm = bgm;
    }

    /// `gmTitleMode_OnEnter`: the preload cache keeps the VS mode's class.
    #[doc(alias = "gmTitleMode_OnEnter")]
    pub fn title_mode_on_enter(&mut self) {
        self.dvd_mut().keep_class();
        self.game.audio.preload_refresh(self.data);
        self.wait_preload_stale_released(crate::lb::heap::ALL_M as i32);
    }

    /// `onExit` (`gmtitlemode.c`): Start goes to the main menu (or a challenger first), else
    /// the attract loop's match. A debug level's other exits aren't retail's.
    pub fn title_mode_on_exit(&mut self) {
        self.game.assert_retail();
        if self.game.title.exit_buttons & i32::from(crate::input::pad::BUTTON_START) != 0 {
            self.menu_entry_unlocks();
        } else {
            self.game.boot_mode.opening_state = 1;
            self.set_pending_game_mode(GM_OPENING_MV);
        }
        self.set_new_game_mode_pending();
    }
}
