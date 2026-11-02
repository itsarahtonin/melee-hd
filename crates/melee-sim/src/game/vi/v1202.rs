//! Giga Bowser defeated (`vi1202.c`, `Vi1202.dat`): the scene's models with Bowser's fallen
//! trophy (`TyKoopaR.dat`) on its stand following one of their joints; no stage, no fighters
//! and no fog, the camera drawn by HSD's own callback.

use melee_data::archive::DataRef;

use super::super::state::Running;
use super::v1201v1::TROPHY_KOOPA;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;

/// `un_8032110C`, `un_80321130`, `un_80321154`, `vi1202_RunFrame`.
pub const STAND_PROC: u32 = 0x8032_110C;
pub const TROPHY_PROC: u32 = 0x8032_1130;
pub const JOBJ_PROC: u32 = 0x8032_1154;
pub const CAMERA_PROC: u32 = 0x8032_1294;

/// The fallen trophy's stand placement (`Toy_803060BC(0x1F, …)`).
const TROPHY_KOOPA_R: i32 = TROPHY_KOOPA + 1;

/// `vi1202.c`'s statics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Vi1202 {
    /// `un_804D7040`: the scene (`visual1202Scene`).
    pub scene: Option<DataRef>,
    /// `un_804D7044`: the stand's scene (`standScene`).
    pub stand: Option<DataRef>,
    /// `un_804D7048`: the trophy's joint (`ToyKoopaRModel_TopN_joint`).
    pub trophy: Option<DataRef>,
    /// `un_804D704C`: the joint the trophy and its stand follow (the last model's third).
    pub anchor: Option<JObjId>,
}

impl Running<'_> {
    /// The camera's process, `vi1202_RunFrame`: its animation; at frame 100 a rumble, at its
    /// last frame the scene's end.
    #[doc(alias = "vi1202_RunFrame")]
    pub fn vi1202_camera_proc(&mut self, gobj: GObjId) {
        let c = self.vi_cobj(gobj);
        crate::hsd::cobj::anim(&mut self.game.objs, self.data, Some(c));
        if self.vi_cobj_frame(c).0 == 100.0 {
            self.vi_rumble(0xE, 0);
        }
        let (cur, end) = self.vi_cobj_frame(c);
        if cur == end {
            self.vi_end();
        }
    }

    /// The scene's entry, `vi1202_Scene_OnEnter`: the sounds and music stopped, the effects,
    /// the scene's song (0x5A) with the audio paused, `Vi1202.dat`, the fallen trophy and the
    /// stand (the region's class 2 trophies marked, `Toy_803124BC`), the lights, the camera
    /// (render links 0, 3, 5, 7 and 11), the models and the scene's sound banks
    /// (`un_80321178`), the trophy and its stand.
    #[doc(alias = "vi1202_Scene_OnEnter")]
    #[doc(alias = "un_80321178")]
    #[doc(alias = "un_8032110C")]
    #[doc(alias = "un_80321130")]
    #[doc(alias = "un_80321154")]
    #[doc(alias = "vi1202_Scene_OnFrame")]
    pub fn vi1202_on_enter(&mut self) {
        self.sfx_stop_all();
        self.lb_audio_music_stop();
        super::super::ef::eflib::init(self);
        super::super::ef::load_sync(self, 0);
        self.lb_play_music(0x5A);
        self.lb_audio_pause(true);
        let vi = self.vi_load("Vi1202.dat");
        let a = vi.archive.expect("Vi1202.dat");
        let scene = super::scene_symbol(self, a, "Vi1202.dat", "visual1202Scene");
        let koopa = self.vi_load("TyKoopaR.dat");
        let a = koopa.archive.expect("TyKoopaR.dat");
        let trophy = super::scene_symbol(self, a, "TyKoopaR.dat", "ToyKoopaRModel_TopN_joint");
        let stnd = self.vi_load("GmRgStnd.dat");
        let a = stnd.archive.expect("GmRgStnd.dat");
        let stand = super::scene_symbol(self, a, "GmRgStnd.dat", "standScene");
        self.mark_class_two_trophies();
        let v = &mut self.game.vi.v1202;
        v.scene = Some(scene);
        v.trophy = Some(trophy);
        v.stand = Some(stand);
        self.vi_lights(scene);
        let cam = self.vi_camera_unanimated(scene, super::super::render::CAMERA_CALLBACK, 8);
        self.game.world.gobj_mut(cam).gxlink_prios = 0x8A9;
        self.vi_camera_animate(cam, scene, CAMERA_PROC);
        self.vi_models(scene, 0xB, JOBJ_PROC, |r, _, j| {
            let [n] = super::super::gm::pause::tree_nodes(&r.game.objs.jobjs, Some(j), &[2]);
            r.game.vi.v1202.anchor = n;
        });
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x18);
        a.lb.want_banks(8, 0x20_u64 << 48);
        a.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
        let anchor = self.game.vi.v1202.anchor;
        self.vi_trophy(trophy, 0.49, anchor, (TROPHY_PROC, 0x17));
        self.vi_stand(stand, TROPHY_KOOPA_R, 0.49, anchor, (STAND_PROC, 0x17));
        self.lb_audio_pause(false);
    }
}
