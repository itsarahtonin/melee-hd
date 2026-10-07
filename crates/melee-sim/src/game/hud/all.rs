//! The HUD as a whole (`melee/if/ifall.c`): its camera and light, where each player's
//! display and the timer go (read off `IfAll.dat`'s layout model), hiding and showing it, and
//! building it at a match's start.

use gekko_fp as fp;
use melee_data::archive::{ArchiveId, DataRef};

use super::super::code;
use super::super::gm::pause::tree_nodes;
use super::super::handles::CodeRef;
use super::super::state::{Game, OBJ_CAMERA, OBJ_LIGHT, Obj, Running};
use super::{Draw, LIGHT_RENDER};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::{jobj, lobj};
use crate::lb::cobj::{CObjDesc, load_cobj};
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// `fn_802F36B8`, the HUD camera's render callback.
pub const CAMERA_PROC: u32 = 0x802F_36B8;

/// The HUD's functions no port covers yet, by address.
mod addr {
    /// `ifMagnify_802FC870`: the off-screen magnifier's setup.
    pub const IFMAGNIFY_802FC870: u32 = 0x802F_C870;
}

/// The archives the HUD loads at a match's start, in the order it loads them.
#[derive(Clone, Copy, Debug)]
pub struct HudArchives {
    /// `IfAll.dat`.
    pub if_all: ArchiveId,
    /// `IfCoGet.dat`, the coin counter's.
    pub if_coget: ArchiveId,
    /// `SdIntro.dat`, the names' font.
    pub sd_intro: ArchiveId,
}

impl Game {
    /// `ifAll_GetHUDGObj`: the HUD camera's GObj.
    #[doc(alias = "ifAll_GetHUDGObj")]
    pub fn hud_gobj(&self) -> Option<GObjId> {
        self.hud.all.gobj
    }

    /// Where the timer goes, `ifAll_GetTimerPosition`.
    #[doc(alias = "ifAll_GetTimerPosition")]
    pub fn timer_position(&self) -> Vec3 {
        self.hud.all.xC
    }

    /// Where player `idx`'s display goes, `ifAll_GetPlayerHUDPosition`.
    #[doc(alias = "ifAll_GetPlayerHUDPosition")]
    pub fn player_hud_position(&self, idx: i32) -> Vec3 {
        self.hud.all.x18[idx as usize]
    }

    /// `ifAll_HideHUD`.
    #[doc(alias = "ifAll_HideHUD")]
    pub fn hide_hud(&mut self) {
        self.hud.hidden = true;
    }

    /// `ifAll_ShowHUD`.
    #[doc(alias = "ifAll_ShowHUD")]
    pub fn show_hud(&mut self) {
        self.hud.hidden = false;
    }

    /// `ifAll_IsHUDHidden`.
    #[doc(alias = "ifAll_IsHUDHidden")]
    pub fn is_hud_hidden(&self) -> bool {
        self.hud.hidden
    }

    /// The HUD camera's render callback, `fn_802F36B8`: everything on the camera's links
    /// unless the HUD is hidden.
    #[doc(alias = "fn_802F36B8")]
    pub fn hud_camera_render(&self, gobj: GObjId) -> Option<Draw> {
        (!self.is_hud_hidden()).then_some(Draw::Camera(gobj, 7))
    }
}

impl Running<'_> {
    /// The world position of the layout model's joint `index` (`lb_80011E24` then
    /// `lb_8000B1CC`).
    fn layout_point(&mut self, root: Option<jobj::JObjId>, index: i32) -> Vec3 {
        let j = tree_nodes(&self.game.objs.jobjs, root, &[index])[0];
        world_position(&mut self.game.objs, self.data, j, None)
    }

    /// Places the players' displays for `count` players, `ifAll_802F343C`: centred for one,
    /// spread for two to four, and closer together (and 0.65 as far from the centre) for
    /// five or six.
    #[doc(alias = "ifAll_802F343C")]
    pub fn place_player_displays(&mut self, count: i32) {
        let root = self.game.hud.all.x8;
        let put = |r: &mut Self, i: usize, index: i32| {
            let p = r.layout_point(root, index);
            r.game.hud.all.x18[i] = p;
        };
        let narrow = |r: &mut Self, i: usize| {
            let v = &mut r.game.hud.all.x18[i];
            v.x = mem(fp::fmuls(reg(v.x), reg(0.65)));
        };
        match count {
            1 => put(self, 0, 9),
            2 => (0..2).for_each(|i| put(self, i, i as i32 + 11)),
            3 => (0..3).for_each(|i| put(self, i, i as i32 + 8)),
            4 => (0..4).for_each(|i| put(self, i, i as i32 + 2)),
            5 | 6 => {
                put(self, 0, 6);
                narrow(self, 0);
                put(self, 5, 7);
                narrow(self, 5);
                for i in 1..5 {
                    put(self, i, i as i32 + 1);
                    narrow(self, i);
                }
            }
            _ => {}
        }
    }

    /// Reads the HUD's layout off its model (the scene's first), `ifAll_802F370C`: the
    /// timer's place, each player's for six, and the alternative places for three and two.
    /// The model's GObj is freed at once; the joints stay, as the layout's root.
    #[doc(alias = "ifAll_802F370C")]
    pub fn read_hud_layout(&mut self, scene: DataRef) {
        let data = self.data;
        let g = self.game.world.create(0xE, 0xF, 0);
        let models = data.ptr(scene).expect("the scene's models");
        let model = data.ptr(models).expect("the scene's first model");
        let root = jobj::load_joint(&mut self.game.objs, data, data.ptr(model));
        self.game.hud.all.x8 = root;
        let p = self.layout_point(root, 0xD);
        self.game.hud.all.xC = p;
        for i in 0..6 {
            let p = self.layout_point(root, i + 2);
            self.game.hud.all.x18[i as usize] = p;
        }
        for i in 0..3 {
            let p = self.layout_point(root, i + 8);
            self.game.hud.all.x60[i as usize] = p;
        }
        for i in 0..2 {
            let p = self.layout_point(root, i + 11);
            self.game.hud.all.x84[i as usize] = p;
        }
        gobj::free(self, g);
    }

    /// Builds the HUD at a match's start, `ifAll_802F390C`: shown, its layout read off
    /// `IfAll.dat`'s `ScInfDmg_scene_data`, its camera and light, then each part's setup.
    #[doc(alias = "ifAll_802F390C")]
    pub fn create_hud(&mut self, files: HudArchives) {
        let data = self.data;
        self.game.show_hud();
        // `lbArchive_80016F80(&archive, "IfAll")`: loaded outside the simulation.
        self.game.hud.archive = Some(files.if_all);
        let scene = DataRef {
            archive: files.if_all,
            offset: data
                .get(files.if_all)
                .public("ScInfDmg_scene_data")
                .expect("IfAll.dat's ScInfDmg_scene_data"),
        };
        self.read_hud_layout(scene);
        let g = &mut *self.game;
        let camera = g.world.create(0x13, 0x14, 0);
        let cameras = data.ptr(scene.at(4)).expect("the scene's cameras");
        let desc = data.ptr(cameras).expect("the scene's first camera");
        let c = load_cobj(&mut g.objs, data, CObjDesc::Data(desc));
        g.world.set_obj(camera, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(camera, CodeRef(CAMERA_PROC), 8, false);
        g.world.gobj_mut(camera).gxlink_prios = 0xD00;
        g.hud.all.gobj = Some(camera);
        let light = g.world.create(0xE, 3, 0);
        let lights = data.ptr(scene.at(8)).expect("the scene's lights");
        let first = data.ptr(lights).expect("the scene's first light");
        let desc = data.ptr(first);
        let l = lobj::load_desc(&mut g.objs, data, desc).expect("the HUD's light");
        g.world.set_obj(light, OBJ_LIGHT, Obj::LObj(l));
        g.world.setup_gx_link(light, CodeRef(LIGHT_RENDER), 0xA, 0);
        g.hud.all.gobj_2 = Some(light);
        self.load_announcements();
        self.load_damage_models();
        self.load_stock_icons();
        self.game.reset_timers();
        self.load_ko_marks();
        code::call(self, addr::IFMAGNIFY_802FC870, &[]);
        self.create_arrows();
        self.game.init_hazard();
        self.init_nametags(files.sd_intro);
        self.init_coin_counter(files.if_coget);
        self.init_scores();
    }

    /// Hides the HUD's parts while a player pauses, `ifAll_802F3394`.
    #[doc(alias = "ifAll_802F3394")]
    pub fn hide_hud_parts(&mut self) {
        self.game.hide_damage_digits();
        self.hide_timers();
        self.game.pause_nametags();
        self.game.magnify_ignore_offscreen(true);
        self.game.pause_hazard();
        self.game.pause_arrows();
        self.game.hide_scores();
    }

    /// Shows them again, `ifAll_802F33CC`.
    #[doc(alias = "ifAll_802F33CC")]
    pub fn show_hud_parts(&mut self) {
        self.game.show_damage_digits();
        self.show_timers();
        self.game.unpause_nametags();
        self.game.magnify_ignore_offscreen(false);
        self.game.unpause_hazard();
        self.game.unpause_arrows();
        self.show_scores();
    }
}

impl Game {
    /// The magnifier ignores (`ifMagnify_802FC8E8`) or again watches (`ifMagnify_802FC940`)
    /// players off the screen, while paused.
    #[doc(alias = "ifMagnify_802FC8E8")]
    #[doc(alias = "ifMagnify_802FC940")]
    pub fn magnify_ignore_offscreen(&mut self, ignore: bool) {
        for p in &mut self.magnify.player {
            p.state.ignore_offscreen = ignore;
        }
    }
}
