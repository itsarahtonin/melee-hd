//! Star Fox's cutscene (`vi0601.c`, `Vi0601.dat`): Corneria set up for its demo (the talk,
//! the Arwings held), three of the stage's Arwings flown by the scene's models at 0.65 their
//! scale, no fighters.

use melee_data::archive::DataRef;

use super::super::state::{Obj, Running};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;
use gekko_fp as fp;

/// `vi_8031E6EC`, `vi0601_GObj_OnRender`, `fn_8031E800`, `vi0601_RunFrame`.
pub const JOBJ_PROC: u32 = 0x8031_E6EC;
pub const CAMERA_RENDER: u32 = 0x8031_E710;
pub const ARWING_PROC: u32 = 0x8031_E800;
pub const CAMERA_PROC: u32 = 0x8031_EB38;

/// `St_Kind_Corneria`.
const ST_KIND_CORNERIA: i32 = 7;

/// `vi0601.c`'s statics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Vi0601 {
    /// `un_804D6FB0`: the scene (`visual0601Scene`).
    pub scene: Option<DataRef>,
    /// `erase_colors_vi0601`: the stage's background color, its alpha a stack byte
    /// `Camera_GetBackgroundColor` never writes (here 0), which nothing reads.
    pub erase: [u8; 4],
}

impl Running<'_> {
    /// The camera's render callback, `vi0601_GObj_OnRender`: through the camera, the screen
    /// erased to the stage's background color, the game camera's layers 2, 1 and 0 over
    /// render links 0 and 3 (then 3 twice), and render links 0, 5, 7 and 11.
    #[doc(alias = "vi0601_GObj_OnRender")]
    pub fn vi0601_camera_render(&mut self, gobj: GObjId) {
        let erase = self.game.vi.v0601.erase;
        self.vi_run_camera_layers(gobj, erase);
    }

    /// Star Fox's and F-Zero's camera drawing: through the camera, the screen erased to
    /// `erase`, the game camera's layers 2, 1 and 0 over render links 0 and 3 (then 3 twice),
    /// and render links 0, 5, 7 and 11, without the fighters' passes.
    pub(crate) fn vi_run_camera_layers(&mut self, gobj: GObjId, erase: [u8; 4]) {
        let c = Some(self.vi_cobj(gobj));
        if crate::hsd::display::camera_begin(&mut self.game.objs, self.data, c) {
            let draws = &mut self.game.objs.display.draws;
            draws.push(crate::hsd::display::Draw::EraseColor(erase));
            draws.push(crate::hsd::display::Draw::EraseWithDepth);
            for (layer, prios) in [(2, 9), (1, 8), (0, 8)] {
                super::super::camera::set_layer(&mut self.game.camera.cam, layer);
                self.game.world.gobj_mut(gobj).gxlink_prios = prios;
                self.render_links(gobj, 7);
            }
            self.game.world.gobj_mut(gobj).gxlink_prios = 0x8A1;
            self.render_links(gobj, 7);
            crate::hsd::display::camera_end(&mut self.game.objs);
        }
    }

    /// An Arwing's process here, `fn_8031E800`: its first child's scale 0.65 times its x, on
    /// every axis.
    #[doc(alias = "fn_8031E800")]
    pub fn vi0601_arwing_proc(&mut self, gobj: GObjId) {
        let child = match self.game.world.gobj(gobj).obj {
            Some(Obj::JObj(j)) => self.game.objs.jobjs[j].child,
            _ => None,
        };
        let child = child.expect("the original's assertion failed: jobj.h: child");
        let x = self.game.objs.jobjs[child].scale.x;
        let s = mem(fp::fmuls(f64::from(0.65_f32), reg(x)));
        jobj::set_scale(&mut self.game.objs.jobjs, child, Vec3::new(s, s, s));
    }

    /// The scene's models, `un_8031E9B8`: the first as a model GObj, then for models 1 to 3
    /// an Arwing of the stage's (`grCorneria_801E1BF0`) whose first child takes the model's
    /// animations, on [`Running::vi0601_arwing_proc`]; and the scene's sound banks.
    #[doc(alias = "un_8031E9B8")]
    #[doc(alias = "vi_8031E6EC")]
    fn vi0601_models(&mut self, scene: DataRef) {
        let first = super::scene_model(self, scene, 0).expect("the scene's first model");
        self.vi_model(first, 0xB, JOBJ_PROC);
        let mut i = 0;
        while let Some(m) = super::scene_model(self, scene, i) {
            if (1..=3).contains(&i) {
                let gobj = super::super::gr::corneria::cutscene_arwing(self);
                let child = match self.game.world.gobj(gobj).obj {
                    Some(Obj::JObj(j)) => self.game.objs.jobjs[j].child,
                    _ => None,
                };
                self.game
                    .world
                    .setup_proc(gobj, super::super::handles::CodeRef(ARWING_PROC), 2);
                let data = self.data;
                super::super::gm::vs::add_model_anim(&mut self.game.objs, data, child, m, 0);
                jobj::req_anim_all(&mut self.game.objs, child, 0.0);
                if let Some(c) = child {
                    self.anim_all(c);
                }
            }
            i += 1;
        }
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x18);
        a.lb.want_banks(8, 8 << 32);
        a.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
    }

    /// The camera's process, `vi0601_RunFrame`: its animation; at frames 537, 559 and 580 a
    /// rumble, at its last frame the scene's end.
    #[doc(alias = "vi0601_RunFrame")]
    pub fn vi0601_camera_proc(&mut self, gobj: GObjId) {
        let c = self.vi_cobj(gobj);
        crate::hsd::cobj::anim(&mut self.game.objs, self.data, Some(c));
        let frame = |r: &Self| f64::from(r.vi_cobj_frame(c).0);
        if frame(self) == 537.0 || frame(self) == 559.0 || frame(self) == 580.0 {
            self.vi_rumble(0x23, 0);
        }
        let (cur, end) = self.vi_cobj_frame(c);
        if cur == end {
            self.vi_end();
        }
    }

    /// The scene's entry, `vi0601_Scene_OnEnter`: the music stopped, the effects, the scene's
    /// song (0x57) with the audio paused, `Vi0601.dat`, the camera, Corneria set up for its
    /// demo, the stage's background color, the models and Arwings, the lights, and the
    /// players reset.
    #[doc(alias = "vi0601_Scene_OnEnter")]
    pub fn vi0601_on_enter(&mut self) {
        self.lb_audio_music_stop();
        super::super::ef::eflib::init(self);
        super::super::ef::load_sync(self, 0);
        self.lb_play_music(0x57);
        self.lb_audio_pause(true);
        let vi = self.vi_load("Vi0601.dat");
        let scene = super::scene_symbol(
            self,
            vi.archive.expect("Vi0601.dat"),
            "Vi0601.dat",
            "visual0601Scene",
        );
        self.game.vi.v0601.scene = Some(scene);
        self.vi_camera(scene, CAMERA_RENDER, 2, CAMERA_PROC);
        self.vi_stage(ST_KIND_CORNERIA, |_| {});
        let k = &self.game.camera.cam.background_color;
        self.game.vi.v0601.erase = [k.r, k.g, k.b, k.a];
        self.vi0601_models(scene);
        self.vi_lights(scene);
        self.game.init_all_players();
        self.lb_audio_pause(false);
    }
}
