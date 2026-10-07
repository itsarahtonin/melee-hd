//! The players' tags (`melee/if/ifnametag.c`): a P1-P4/CP marker over each fighter, coloured
//! by port or team, following the fighter's screen position, with the player's name tag
//! (from the memory card's names) as text above it on the HUD's text canvas.

use gekko_fp as fp;
use melee_data::archive::{ArchiveId, DataRef};

use super::super::code;
use super::super::handles::CodeRef;
use super::super::pl::player::{PKIND_HUMAN, PKIND_NONE};
use super::super::state::{Game, OBJ_CAMERA, OBJ_JOBJ, Obj, Running, UserData};
use super::super::structs::StaticModelDesc;
use super::{Draw, HIDDEN};
use crate::fpu::{mem, reg};
use crate::hsd::cobj::StaticDesc;
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj;
use crate::hsd::sis::TextId;
use crate::lb::cobj::{CObjDesc, load_cobj};
use crate::vec::Vec3;

/// The tags' process and render callbacks.
pub const PROC: u32 = 0x802F_CC44;
pub const RENDER: u32 = 0x802F_CAA4;
pub const CAMERA_RENDER: u32 = 0x802F_CAC4;

/// `NAMETAG_DISABLED`.
const NO_NAMETAG: u8 = 0x78;

/// `GetNameText`, the memory card's names (the name entry screen's port's).
const GET_NAME_TEXT: &str = "GetNameText";

/// The marker frames (`un_803F98B8`): by port (or computer) and team.
const FRAMES: [[f32; 5]; 5] = [
    [0.0, 1.0, 3.0, 0.0, 16.0],
    [4.0, 5.0, 7.0, 5.0, 17.0],
    [8.0, 9.0, 11.0, 10.0, 18.0],
    [12.0, 13.0, 15.0, 15.0, 19.0],
    [16.0, 17.0, 19.0, 18.0, 18.0],
];
/// The hearts' frames by port (`un_803F991C`).
const HEARTS: [f32; 4] = [24.0, 25.0, 26.0, 27.0];

/// `nametag_CObjDesc`: the tags' orthographic camera.
const CAMERA_DESC: StaticDesc = StaticDesc {
    flags: 0,
    projection_type: 3,
    viewport: [0, 640, 0, 480],
    scissor: [0, 640, 0, 480],
    eye: Vec3::new(0.0, 0.0, 300.0),
    interest: Vec3::new(0.0, 0.0, 0.0),
    roll: 0.0,
    near: 0.1,
    far: 32768.0,
    projection: [0.0, -480.0, 0.0, 640.0],
};

/// The tags' statics (`ifnametag.c`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NameTags {
    /// `un_804A1ED0`: the marker model.
    pub model: StaticModelDesc,
    /// `un_804A1EE0`: each slot's marker.
    pub gobjs: [Option<GObjId>; 6],
    /// `un_804A1EF8`: each slot's name entry in the text.
    pub entries: [i32; 6],
    /// `un_804D6D68`: the tags' camera.
    pub camera: Option<GObjId>,
    /// `un_804D6D6C`: hidden while paused.
    pub hidden: u8,
    /// `un_804D6D70`: shown regardless (Ready's start to GO!'s end).
    pub shown: [u8; 6],
    /// `un_804D6D78` and `un_804D6D7C`: the names' text and its canvas.
    pub text: Option<TextId>,
    pub canvas: i32,
}

impl Game {
    /// A slot's marker frame: by port, by team in a team match (a computer teammate's heart
    /// on the first player's team with the rules' `x4_2`), or the computer's,
    /// `un_802FC9B4`.
    #[doc(alias = "un_802FC9B4")]
    pub fn nametag_frame(&self, slot: u8, team: u8, teams: u8, pkind: u8) -> f32 {
        let port = self.player_port(i32::from(slot)) as u8 as usize;
        let row = |r: usize| {
            *FRAMES
                .get(r)
                .and_then(|r| r.get(usize::from(team)))
                .unwrap_or_else(|| panic!("a tag frame past the table isn't ported"))
        };
        if teams != 0 {
            if pkind == 0 {
                return row(port);
            }
            if (self.vs_fighter_x4_b3(port as i32) || self.vs_x4_2())
                && i32::from(team) == self.player_team(0)
            {
                return HEARTS[self.player_port(0) as usize];
            }
            return row(4);
        }
        if pkind == 0 {
            return FRAMES[port][3];
        }
        18.0
    }

    /// Whether a slot shows a name (a human with a tag).
    fn has_nametag(&self, slot: i32) -> bool {
        self.player_pkind(slot) == PKIND_HUMAN && self.player_nametag(slot) != NO_NAMETAG
    }

    fn nametag_frame_of(&self, slot: i32) -> f32 {
        self.nametag_frame(
            slot as u8,
            self.player_team(slot) as u8,
            u8::from(self.vs_is_teams()),
            self.player_pkind(slot) as u8,
        )
    }

    /// A marker's render callback, `NameTag_RenderCallback`.
    #[doc(alias = "NameTag_RenderCallback")]
    pub fn nametag_render(&self, gobj: GObjId, pass: i32) -> Option<Draw> {
        Some(Draw::JObjs(gobj, pass))
    }

    /// Shows every marker regardless (Ready), `un_802FD404`.
    #[doc(alias = "un_802FD404")]
    pub fn show_nametags(&mut self) {
        self.hud.nametags.shown = [1; 6];
    }

    /// `un_802FD428`.
    #[doc(alias = "un_802FD428")]
    pub fn unshow_nametags(&mut self) {
        self.hud.nametags.shown = [0; 6];
    }

    /// `un_802FD450`: hidden while paused.
    #[doc(alias = "un_802FD450")]
    pub fn pause_nametags(&mut self) {
        self.hud.nametags.hidden = 1;
    }

    /// `un_802FD45C`.
    #[doc(alias = "un_802FD45C")]
    pub fn unpause_nametags(&mut self) {
        self.hud.nametags.hidden = 0;
    }

    /// The marker model from `IfAll.dat`'s `ScInfPnm_scene_models`, `un_802FCBA0`.
    #[doc(alias = "un_802FCBA0")]
    pub fn load_nametag_model(&mut self, data: &melee_data::archive::Archives) {
        let a = self.hud.archive.expect("IfAll.dat");
        let at = data
            .get(a)
            .public("ScInfPnm_scene_models")
            .expect("IfAll.dat's ScInfPnm_scene_models");
        let x = data
            .ptr(DataRef {
                archive: a,
                offset: at,
            })
            .expect("a model");
        let m = &mut self.hud.nametags.model;
        m.joint = data.ptr(x);
        let first = |o: u32| data.ptr(x.at(o)).and_then(|l| data.ptr(l));
        if data.ptr(x.at(4)).is_some() {
            m.animjoint = first(4);
        }
        if data.ptr(x.at(8)).is_some() {
            m.matanim_joint = first(8);
        }
        if data.ptr(x.at(0xC)).is_some() {
            m.shapeanim_joint = first(0xC);
        }
    }
}

impl Running<'_> {
    /// The tags' camera's render callback, `fn_802FCAC4`: with the HUD hidden or the tags
    /// paused, the names move off the screen (text state the display pass changes); else
    /// everything on the camera's links.
    #[doc(alias = "fn_802FCAC4")]
    pub fn nametag_camera_render(&mut self, gobj: GObjId) -> Option<Draw> {
        if self.game.is_hud_hidden() || self.game.hud.nametags.hidden != 0 {
            for i in 0..6 {
                if self.game.has_nametag(i) {
                    self.move_name(i, -5000.0, 0.0);
                }
            }
            None
        } else {
            Some(Draw::Camera(gobj, 7))
        }
    }

    fn move_name(&mut self, slot: i32, x: f64, y: f64) {
        let t = self.game.hud.nametags.text.expect("the names' text");
        let k = self.game.hud.nametags.entries[slot as usize];
        self.game.sis.set_position(self.data, t, k, x, y);
    }

    /// A marker's process, `fn_802FCC44`: shown over a playing fighter with stocks left (if
    /// the player has a tag or the markers show regardless), placed at the fighter's screen
    /// position above its head, the name above that.
    #[doc(alias = "fn_802FCC44")]
    pub fn nametag_proc(&mut self, gobj: GObjId) {
        let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj else {
            panic!("a tag without its model");
        };
        let Some(UserData::NameTagSlot(slot)) = self.game.world.gobj(gobj).user_data else {
            panic!("a tag without its slot");
        };
        let s = i32::from(slot);
        let g = &*self.game;
        let child = g.objs.jobjs[j].child;
        let show = g.player_pkind(s) != PKIND_NONE
            && g.player_state(s) != 0
            && g.player_stocks(s) != 0
            && (g.hud.nametags.shown[usize::from(slot)] != 0
                || g.player_nametag(s) != NO_NAMETAG
                || g.player_80036058(s)
                || g.vs_fighter_x4_b3(s));
        if !show {
            jobj::set_flags(&mut self.game.objs.jobjs, child, HIDDEN);
            if self.game.has_nametag(s) {
                self.move_name(s, -5000.0, 0.0);
            }
            return;
        }
        jobj::clear_flags(&mut self.game.objs.jobjs, child, HIDDEN);
        let mut v = self.game.player_pos(s);
        let h = self.player_nametag_height(s);
        v.y = mem(fp::fadds(reg(v.y), h));
        v.y = mem(fp::fsubs(reg(v.y), 2.5));
        let cam = super::super::camera::gobj(&self.game.camera.cam).expect("the camera's GObj");
        let Some(Obj::CObj(c)) = self.game.world.gobj(cam).obj else {
            panic!("the camera's GObj without its CObj");
        };
        let sp = crate::lb::screen::world_to_screen(&mut self.game.objs, self.data, c, v, false)
            .expect("a screen position");
        let jobjs = &mut self.game.objs.jobjs;
        jobj::set_srt(jobjs, j, |o| o.translate.x = sp.x);
        let y = mem(fp::fneg(reg(sp.y)));
        jobj::set_srt(jobjs, j, |o| o.translate.y = y);
        if self.game.has_nametag(s) {
            self.move_name(s, reg(sp.x), fp::fsubs(reg(sp.y), 56.0));
        }
    }

    /// A slot's name entry in the text (the inline `createNameText`).
    fn create_name_text(&mut self, _slot: i32) {
        code::unported_call(GET_NAME_TEXT);
    }

    /// A slot's marker, `NameTag_Create`.
    #[doc(alias = "NameTag_Create")]
    pub fn create_nametag(&mut self, slot: i32) {
        let data = self.data;
        let g = &mut *self.game;
        let o = g.world.create(0xE, 15, 0);
        g.hud.nametags.gobjs[slot as usize] = Some(o);
        let m = g.hud.nametags.model.clone();
        let j = jobj::load_joint(&mut g.objs, data, m.joint).expect("the tag's model");
        g.world.set_obj(o, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(o, CodeRef(RENDER), 9, 0);
        jobj::set_srt(&mut g.objs.jobjs, j, |o| o.scale.x = 10.0);
        jobj::set_srt(&mut g.objs.jobjs, j, |o| o.scale.y = 10.0);
        jobj::set_srt(&mut g.objs.jobjs, j, |o| o.scale.z = 10.0);
        jobj::add_anim_all(
            &mut g.objs,
            data,
            Some(j),
            m.animjoint,
            m.matanim_joint,
            m.shapeanim_joint,
        );
        let mut f = g.nametag_frame_of(slot);
        if g.has_nametag(slot) {
            let q = fp::fctiwz(crate::fpu::reg(f));
            f = if q >= 16 {
                28.0
            } else {
                mem(fp::frsp(f64::from(q % 4 + 20)))
            };
            self.create_name_text(slot);
        }
        let g = &mut *self.game;
        jobj::req_anim_all(&mut g.objs, Some(j), f);
        jobj::anim_all(
            &mut g.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            Some(j),
        );
        g.world
            .init_user_data(o, 0, UserData::NameTagSlot(slot as u8));
        g.world.setup_proc(o, CodeRef(PROC), 17);
    }

    /// A slot's marker posed afresh (its name made again), `un_802FD28C`.
    #[doc(alias = "un_802FD28C")]
    pub fn refresh_nametag(&mut self, slot: i32) {
        let data = self.data;
        let o = self.game.hud.nametags.gobjs[slot as usize].expect("the tag's GObj");
        let Some(Obj::JObj(j)) = self.game.world.gobj(o).obj else {
            panic!("a tag without its model");
        };
        let mut f = self.game.nametag_frame_of(slot);
        if self.game.has_nametag(slot) {
            let q = fp::fctiwz(crate::fpu::reg(f));
            f = if q >= 16 {
                28.0
            } else {
                mem(fp::frsp(f64::from(q % 4 + 20)))
            };
            let t = self.game.hud.nametags.text.expect("the names' text");
            let k = self.game.hud.nametags.entries[slot as usize];
            self.game.sis.clear_string(data, t, k);
            self.create_name_text(slot);
        }
        jobj::req_anim_all(&mut self.game.objs, Some(j), f);
        jobj::anim_all(
            &mut self.game.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            Some(j),
        );
    }

    /// Frees the markers and their camera, `un_802FD468`.
    #[doc(alias = "un_802FD468")]
    pub fn free_nametags(&mut self) {
        for i in 0..6 {
            if let Some(o) = self.game.hud.nametags.gobjs[i] {
                gobj::free(self, o);
            }
        }
        let c = self.game.hud.nametags.camera.expect("the tags' camera");
        gobj::free(self, c);
    }

    /// The tags at the HUD's setup: their camera, the names' canvas and text (font 2,
    /// `SdIntro.dat`'s strings, loaded outside the simulation), the model and each slot's
    /// marker, `un_802FD4C8`.
    #[doc(alias = "un_802FD4C8")]
    pub fn init_nametags(&mut self, sd_intro: ArchiveId) {
        let data = self.data;
        let g = &mut *self.game;
        let n = &mut g.hud.nametags;
        n.gobjs = [None; 6];
        n.camera = None;
        n.hidden = 0;
        n.shown = [0; 6];
        let o = g.world.create(0xE, 15, 0);
        g.hud.nametags.camera = Some(o);
        let c = load_cobj(&mut g.objs, data, CObjDesc::Static(&CAMERA_DESC));
        g.world.set_obj(o, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(o, CodeRef(CAMERA_RENDER), 6, false);
        g.world.gobj_mut(o).gxlink_prios = 0x200;
        let canvas = self.add_canvas(2, Some(o), 14, 15, 0, 9, 6, 0);
        self.game.hud.nametags.canvas = canvas;
        self.load_font(2, sd_intro, "SIS_IntroData");
        let t = self.create_text_buffer(2, canvas);
        self.game.hud.nametags.text = Some(t);
        let tx = self.game.sis.text_mut(t);
        tx.pos.z = -10.0;
        tx.default_alignment = 1;
        self.game.load_nametag_model(data);
        for i in 0..6 {
            self.create_nametag(i);
        }
    }
}
